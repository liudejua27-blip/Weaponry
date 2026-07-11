import React from 'react'
import ReactDOM from 'react-dom/client'
import { App } from './app/App'
import { JobEventProvider } from './app/providers/JobEventProvider'
import { RuntimeProvider } from './app/providers/RuntimeProvider'
import { SelectionProvider } from './app/providers/SelectionProvider'
import './styles.css'

ReactDOM.createRoot(document.getElementById('root') as HTMLElement).render(
  <React.StrictMode>
    <RuntimeProvider>
      <JobEventProvider>
        <SelectionProvider>
          <App />
        </SelectionProvider>
      </JobEventProvider>
    </RuntimeProvider>
  </React.StrictMode>,
)
