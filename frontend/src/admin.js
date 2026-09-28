import './admin.scss'
import { start } from './Admin.elm'

const root = document.getElementById('admin-root')
if (root) {
  const app = start(root, { path: window.location.pathname })
  if (app.ports?.pushUrl) {
    app.ports.pushUrl.subscribe((url) => {
      history.pushState({}, '', url)
      window.scrollTo(0, 0)
    })
  }
  window.addEventListener('popstate', () => {
    app.ports?.urlChanged?.send(window.location.pathname)
  })
}

const dialogs = new MutationObserver(() => {
  document.querySelectorAll('dialog.modal').forEach((dialog) => {
    if (dialog.getAttribute('data-open') === 'false') {
      if (dialog.open) dialog.close()
      return
    }
    if (!dialog.open) dialog.showModal()
  })
})
dialogs.observe(document.body, { childList: true, subtree: true, attributes: true, attributeFilter: ['data-open'] })
