import React from 'react'
import ReactDOM from 'react-dom/client'
import { App } from './app/App'
import { RuntimeProvider } from './app/providers/RuntimeProvider'
import './styles.css'

ReactDOM.createRoot(document.getElementById('root') as HTMLElement).render(
  <React.StrictMode>
    <RuntimeProvider>
      <App />
    </RuntimeProvider>
  </React.StrictMode>,
)
