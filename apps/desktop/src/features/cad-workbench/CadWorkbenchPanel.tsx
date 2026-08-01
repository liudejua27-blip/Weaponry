import { WorkbenchShell } from './WorkbenchShell.js'

/**
 * Product composition boundary. Runtime state, Snapshot ownership and the
 * single viewport remain in WorkbenchShell; this stable entry keeps imports
 * and legacy harnesses pointed at the CAD workbench feature.
 */
export function CadWorkbenchPanel() {
  return <WorkbenchShell />
}
