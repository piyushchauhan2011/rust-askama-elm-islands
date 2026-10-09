import subprocess
import time
import threading
import json
import re

def parse_mem_to_mib(mem_str):
    # e.g. "140.4MiB / 7.748GiB" or "1.2GiB"
    match = re.search(r'([0-9.]+)\s*([A-Za-z]+)', mem_str)
    if not match:
        return 0.0
    val, unit = float(match.group(1)), match.group(2).lower()
    if 'kib' in unit or 'kb' in unit:
        return val / 1024.0
    elif 'mib' in unit or 'mb' in unit:
        return val
    elif 'gib' in unit or 'gb' in unit:
        return val * 1024.0
    elif 'b' in unit:
        return val / (1024.0 * 1024.0)
    return val

def parse_cpu(cpu_str):
    # e.g. "12.34%"
    clean = cpu_str.replace('%', '').strip()
    try:
        return float(clean)
    except:
        return 0.0

def get_container_stats(container_name):
    cmd = ["docker", "stats", "--no-stream", "--format", "{{.CPUPerc}},{{.MemUsage}}", container_name]
    res = subprocess.run(cmd, capture_output=True, text=True)
    if res.returncode == 0 and res.stdout.strip():
        parts = res.stdout.strip().split(',', 1)
        if len(parts) == 2:
            cpu = parse_cpu(parts[0])
            mem = parse_mem_to_mib(parts[1])
            return cpu, mem
    return 0.0, 0.0

def reset_database():
    subprocess.run([
        "docker", "compose", "exec", "-T", "postgres", "psql", "-U", "hotel", "-d", "hotel", "-c",
        "DELETE FROM page_snapshots; DELETE FROM snapshot_jobs;"
    ], check=True, capture_output=True)
    subprocess.run(["cargo", "run", "--bin", "seed"], check=True, capture_output=True)

def benchmark_browser(name, container_name, cdp_url):
    print(f"\n==========================================")
    print(f"Starting Benchmark: {name}")
    print(f"Container: {container_name}")
    print(f"CDP URL: {cdp_url}")
    print(f"==========================================")

    reset_database()
    baseline_cpu, baseline_mem = get_container_stats(container_name)
    print(f"Baseline Idle Memory: {baseline_mem:.2f} MiB")

    samples = []
    stop_event = threading.Event()

    def poller():
        while not stop_event.is_set():
            cpu, mem = get_container_stats(container_name)
            samples.append((time.time(), cpu, mem))
            time.sleep(0.15)

    poll_thread = threading.Thread(target=poller, daemon=True)
    poll_thread.start()

    start_time = time.time()
    res = subprocess.run(
        ["node", "snapshot-worker/worker.mjs", "--once"],
        env={
            **subprocess.os.environ,
            "LIGHTPANDA_CDP_URL": cdp_url,
            "SNAPSHOT_API_ORIGIN": "http://host.docker.internal:5000",
            "SNAPSHOT_CONCURRENCY": "1",
        },
        capture_output=True,
        text=True
    )
    elapsed = time.time() - start_time
    stop_event.set()
    poll_thread.join(timeout=2.0)

    if res.returncode != 0:
        print(f"FAILED: {res.stderr}")
        return None

    # Verify count in database
    count_res = subprocess.run([
        "docker", "compose", "exec", "-T", "postgres", "psql", "-U", "hotel", "-d", "hotel", "-t", "-c",
        "SELECT count(*) FROM page_snapshots;"
    ], capture_output=True, text=True)
    published_count = int(count_res.stdout.strip()) if count_res.returncode == 0 else 0

    cpu_vals = [s[1] for s in samples] if samples else [0.0]
    mem_vals = [s[2] for s in samples] if samples else [baseline_mem]

    max_cpu = max(cpu_vals)
    avg_cpu = sum(cpu_vals) / len(cpu_vals)
    peak_mem = max(mem_vals)
    mem_delta = peak_mem - baseline_mem

    print(f"Completed {published_count} snapshots in {elapsed:.2f}s ({elapsed*1000/published_count:.1f}ms/snapshot)")
    print(f"Peak Memory: {peak_mem:.2f} MiB (Delta: +{mem_delta:.2f} MiB)")
    print(f"Max CPU: {max_cpu:.1f}%, Avg CPU: {avg_cpu:.1f}%")

    return {
        "name": name,
        "container": container_name,
        "elapsed_seconds": elapsed,
        "snapshots": published_count,
        "ms_per_snapshot": (elapsed * 1000 / published_count) if published_count else 0,
        "baseline_mem_mib": baseline_mem,
        "peak_mem_mib": peak_mem,
        "mem_delta_mib": mem_delta,
        "max_cpu_pct": max_cpu,
        "avg_cpu_pct": avg_cpu,
    }

if __name__ == "__main__":
    results = []
    # 1. Lightpanda
    lp_res = benchmark_browser("Lightpanda Browser (Zig)", "rust-askama-elm-app-lightpanda-1", "ws://127.0.0.1:9222")
    if lp_res:
        results.append(lp_res)

    time.sleep(2)

    # 2. Playwright Docker
    pw_res = benchmark_browser("Playwright Chromium (Docker)", "playwright-chromium", "http://127.0.0.1:9223")
    if pw_res:
        results.append(pw_res)

    print("\n\n" + "="*80)
    print("FINAL BENCHMARK SUMMARY")
    print("="*80)
    print(json.dumps(results, indent=2))
