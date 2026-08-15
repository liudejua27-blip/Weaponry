import { RuntimeViewer } from './features/runtime-viewer/RuntimeViewer'

/**
 * ForgeCAD Desktop is a read-only projection surface for the Codex-led
 * Runtime.  Design intent, reference attachment, action approval and
 * permanent writes stay in Codex/MCP; this entry point must not recreate a
 * chat, upload wizard or fake "generate" action that is disconnected from
 * Runtime truth.
 */
export default function App() {
  return <RuntimeViewer />
}
