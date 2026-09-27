import './styles.scss'
import { start } from './Islands.elm'

document.documentElement.classList.add('js-enabled')

const header = document.querySelector('.site-header[data-overlay="true"]')
const sentinel = document.querySelector('.site-header__sentinel')
if (header && sentinel && 'IntersectionObserver' in window) {
  const scrim = header.querySelector('.site-header__scrim')
  const brand = header.querySelector('.brand')
  const search = header.querySelector('.site-header__search-button')
  const observer = new IntersectionObserver(([entry]) => {
    const solid = !entry.isIntersecting
    header.classList.toggle('site-header--solid', solid)
    header.classList.toggle('site-header--overlay', !solid)
    scrim?.classList.toggle('site-header__scrim--hidden', solid)
    brand?.classList.toggle('brand--light', !solid)
    search?.classList.toggle('is-primary', solid)
    search?.classList.toggle('site-header__search-button--overlay', !solid)
  })
  observer.observe(sentinel)
}

const markers = [...document.querySelectorAll('[data-island]')]
const sourceSnapshot = window.location.pathname === '/_snapshot-source'
let pending = sourceSnapshot ? markers.length : 0

function markReady() {
  if (sourceSnapshot && --pending <= 0) window.__SNAPSHOT_READY__ = true
}

if (sourceSnapshot && pending === 0) window.__SNAPSHOT_READY__ = true

const started = new WeakSet()

function mount(marker) {
  if (started.has(marker)) return
  started.add(marker)
  try {
    const props = JSON.parse(marker.dataset.props || '{}')
    if (marker.dataset.island === 'MobileNav') {
      props.path =
        window.location.pathname === '/_snapshot-source'
          ? new URLSearchParams(window.location.search).get('path') || '/'
          : window.location.pathname
    }
    const host = document.createElement('div')
    marker.replaceChildren(host)
    start(host, { island: marker.dataset.island || '', props })
    if (sourceSnapshot) markReady()
  } catch (cause) {
    const error = cause instanceof Error ? cause : new Error(String(cause))
    console.error(error)
    window.dispatchEvent(new ErrorEvent('error', { error, message: error.message }))
  }
}

const dialogs = new MutationObserver(() => {
  document.querySelectorAll('dialog.modal').forEach((dialog) => {
    if (!dialog.open) dialog.showModal()
  })
})
dialogs.observe(document.body, { childList: true, subtree: true })

const deferred = markers.filter((marker) => marker.dataset.hydrate === 'visible')
markers.filter((marker) => marker.dataset.hydrate !== 'visible').forEach(mount)
if (sourceSnapshot) {
  deferred.forEach(mount)
} else if (deferred.length && 'IntersectionObserver' in window) {
  const observer = new IntersectionObserver((entries) => {
    for (const entry of entries) {
      if (!entry.isIntersecting) continue
      observer.unobserve(entry.target)
      mount(entry.target)
    }
  })
  deferred.forEach((marker) => observer.observe(marker))
} else {
  deferred.forEach(mount)
}
