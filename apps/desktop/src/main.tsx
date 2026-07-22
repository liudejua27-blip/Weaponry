import React from 'react'
import ReactDOM from 'react-dom/client'
import { App } from './app/App'
import { RuntimeProvider } from './app/providers/RuntimeProvider'
import { runPackagedK001ProbeOnce } from './shared/api/packagedK001Probe'
import { runPackagedK002ProbeOnce } from './shared/api/packagedK002Probe'
import { runPackagedArmWebviewQaOnce } from './shared/api/packagedArmWebviewQa'
import './styles.css'

void (async () => {
  await runPackagedK001ProbeOnce()
  await runPackagedK002ProbeOnce()
  await runPackagedArmWebviewQaOnce()
})()

ReactDOM.createRoot(document.getElementById('root') as HTMLElement).render(
  <React.StrictMode>
    <RuntimeProvider>
      <App />
    </RuntimeProvider>
  </React.StrictMode>,
)
