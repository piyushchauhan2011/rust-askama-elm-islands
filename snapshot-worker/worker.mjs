import { createHash, randomUUID } from 'node:crypto'
import { readFileSync, existsSync, readdirSync } from 'node:fs'
import { chromium } from 'playwright-core'
import pg from 'pg'

if (existsSync('.env')) {
  for (const line of readFileSync('.env', 'utf8').split(/\r?\n/)) {
    const match = /^([A-Z_]+)=(.*)$/.exec(line.trim())
    if (match && process.env[match[1]] === undefined) process.env[match[1]] = match[2]
  }
}
const { DATABASE_URL, SNAPSHOT_INTERNAL_TOKEN } = process.env
if (!DATABASE_URL || !SNAPSHOT_INTERNAL_TOKEN || SNAPSHOT_INTERNAL_TOKEN.length < 32) throw new Error('DATABASE_URL and a 32+ character SNAPSHOT_INTERNAL_TOKEN are required.')
const origin = (process.env.SNAPSHOT_API_ORIGIN || 'http://localhost:5000').replace(/\/$/, '')
const manifest = readFileSync('static/assets/.vite/manifest.json')
const fingerprint = createHash('sha256').update(manifest)
// Canonical links are stored in the HTML; an origin change requires new snapshots.
fingerprint.update('\0PUBLIC_ORIGIN\0').update(process.env.PUBLIC_ORIGIN || '')
for (const directory of ['templates', 'src', 'frontend/src', 'snapshot-worker']) {
  function include(dir) {
    for (const file of readdirSync(dir, { withFileTypes: true }).sort((a, b) => a.name.localeCompare(b.name))) {
      const path = `${dir}/${file.name}`
      if (file.isDirectory()) include(path)
      else if (/\.(rs|html|elm|mjs|js|scss)$/.test(file.name)) {
        fingerprint.update(path).update(readFileSync(path))
      }
    }
  }
  include(directory)
}
const assetVersion = fingerprint.digest('hex')
const assets = JSON.parse(manifest)
const runtime = '/assets/' + assets['src/islands.js'].file
const once = process.argv.includes('--once')
const drain = process.argv.includes('--drain')
const pool = new pg.Pool({ connectionString: DATABASE_URL, max: 4 })
const browser = await chromium.connectOverCDP(process.env.LIGHTPANDA_CDP_URL || 'ws://127.0.0.1:9222')
let closing = false
process.on('SIGTERM', () => { closing = true })
process.on('SIGINT', () => { closing = true })

async function releaseChangedAssets() {
  // Razor markup, query rendering, worker serialization and Vite assets form one release.
  const version = randomUUID().replaceAll('-', '')
  await pool.query(`INSERT INTO snapshot_jobs (path, desired_version, attempts, next_attempt_at, leased_until)
    SELECT path, $1, 0, now(), NULL FROM page_snapshots WHERE asset_version <> $2
    ON CONFLICT (path) DO UPDATE SET desired_version = EXCLUDED.desired_version,
      attempts = 0, next_attempt_at = now(), leased_until = NULL`, [version, assetVersion])
}
async function claim() {
  const { rows } = await pool.query(`WITH next AS (
    SELECT path FROM snapshot_jobs WHERE next_attempt_at <= now()
      AND (leased_until IS NULL OR leased_until < now())
    ORDER BY next_attempt_at, path LIMIT 1 FOR UPDATE SKIP LOCKED
  ) UPDATE snapshot_jobs AS job SET leased_until = now() + interval '90 seconds'
    FROM next WHERE job.path = next.path RETURNING job.path, job.desired_version`, [])
  return rows[0]
}
async function render(job) {
  const context = await browser.newContext({ viewport: { width: 1440, height: 900 }, reducedMotion: 'reduce', extraHTTPHeaders: { 'X-Snapshot-Token': SNAPSHOT_INTERNAL_TOKEN } })
  try {
    await context.route('**/*', route => {
      if (new URL(route.request().url()).origin !== origin) return route.abort()
      return route.continue()
    })
    const page = await context.newPage()
    const failures = []
    page.on('pageerror', error => failures.push(error.message))
    page.on('console', message => {
      if (message.type() === 'error') failures.push(`console: ${message.text()}`)
    })
    page.on('response', response => { if (response.status() >= 400) failures.push(`${response.status()} ${response.url()}`) })
    const response = await page.goto(`${origin}/_snapshot-source?path=${encodeURIComponent(job.path)}`, { waitUntil: 'domcontentloaded', timeout: 30000 })
    if (!response) throw new Error(`No response from source: ${job.path}`)
    if (response.status() !== 200) throw new Error(`Source returned HTTP ${response.status()}: ${job.path}`)
    await page.waitForFunction(() => window.__SNAPSHOT_READY__ === true, undefined, { timeout: 30000 })
    if (failures.length) throw new Error(`Browser errors for ${job.path}: ${failures.join('; ').slice(0, 400)}`)
    const result = await page.evaluate(expectedRuntime => {
      const canonical = document.querySelector('link[rel="canonical"]')?.getAttribute('href')
      const h1 = document.querySelectorAll('h1').length
      const islands = [...document.querySelectorAll('[data-island]')]
      const executableScripts = [...document.querySelectorAll('script[src]')].map(node => node.getAttribute('src'))
      const csrf = document.querySelector('input[name="__RequestVerificationToken"], input[name="password"]')
      const root = document.documentElement.cloneNode(true)
      // Vite inserts hints while rendering islands. Persisting them would eagerly
      // fetch every captured chunk before the visitor actually needs it.
      root.querySelectorAll('link[rel="modulepreload"]').forEach(link => link.remove())
      // CSR creates adjacent text nodes; HTML parsing would coalesce them. React hydration
      // needs the same boundaries that renderToString normally marks with empty comments.
      for (const island of root.querySelectorAll('[data-island]')) {
        const walker = document.createTreeWalker(island, NodeFilter.SHOW_TEXT)
        const adjacent = []
        while (walker.nextNode()) {
          const node = walker.currentNode
          if (node.previousSibling?.nodeType === Node.TEXT_NODE) adjacent.push(node)
        }
        for (const node of adjacent) node.before(document.createComment(''))
      }
      return { html: '<!doctype html>\n' + root.outerHTML, canonical, h1,
        islandCount: islands.length, unrendered: islands.filter(node => !node.firstElementChild).length,
        executableScripts, hasPrivateForm: Boolean(csrf), expectedRuntime }
    }, runtime)
    if (!result.canonical || new URL(result.canonical).pathname !== job.path || result.h1 !== 1 || result.unrendered || result.hasPrivateForm ||
      result.executableScripts.length !== 1 || result.executableScripts[0] !== runtime || result.html.includes(SNAPSHOT_INTERNAL_TOKEN))
      throw new Error(`Invalid snapshot for ${job.path}: ${JSON.stringify({ canonical: result.canonical, h1: result.h1, unrendered: result.unrendered, scripts: result.executableScripts })}`)
    return result.html
  } finally { await context.close() }
}
async function publish(job, html) {
  const client = await pool.connect()
  try {
    await client.query('BEGIN')
    const { rows } = await client.query('SELECT desired_version FROM snapshot_jobs WHERE path = $1 FOR UPDATE', [job.path])
    if (rows[0]?.desired_version !== job.desired_version) { await client.query('ROLLBACK'); return }
    await client.query(`INSERT INTO page_snapshots (path, html, asset_version, generated_at) VALUES ($1,$2,$3,now())
      ON CONFLICT (path) DO UPDATE SET html=EXCLUDED.html, asset_version=EXCLUDED.asset_version, generated_at=now()`, [job.path, html, assetVersion])
    await client.query('DELETE FROM snapshot_jobs WHERE path = $1', [job.path])
    await client.query('COMMIT')
    console.log(`Published ${job.path} (${assetVersion.slice(0, 10)})`)
  } catch (error) { await client.query('ROLLBACK'); throw error }
  finally { client.release() }
}
async function fail(job, error) {
  console.error(`Snapshot ${job.path}:`, error)
  await pool.query(`UPDATE snapshot_jobs SET attempts = attempts + 1, leased_until = NULL,
    next_attempt_at = now() + (LEAST(3600, POWER(2, LEAST(attempts, 11)))::text || ' seconds')::interval
    WHERE path = $1 AND desired_version = $2`, [job.path, job.desired_version])
}
async function work() {
  while (!closing) {
    const job = await claim()
    if (!job) return false
    try { await publish(job, await render(job)) }
    catch (error) { await fail(job, error) }
  }
  return false
}
try {
  await releaseChangedAssets()
  const concurrency = parseInt(process.env.SNAPSHOT_CONCURRENCY || '1', 10)
  do {
    if (concurrency > 1) {
      await Promise.all(Array.from({ length: concurrency }, () => work()))
    } else {
      await work()
    }
    if (once) break
    await new Promise(resolve => setTimeout(resolve, 1500))
  } while (!closing)
  if (drain) {
    const { rows } = await pool.query('SELECT count(*)::int AS remaining FROM snapshot_jobs')
    if (rows[0].remaining > 0) throw new Error(`${rows[0].remaining} snapshots remain unpublished.`)
  }
} finally {
  await browser.close()
  await pool.end()
}
