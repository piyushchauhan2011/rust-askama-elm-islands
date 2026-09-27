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
