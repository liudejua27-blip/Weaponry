export type F026IconName =
  | 'add'
  | 'style'
  | 'material'
  | 'reference'
  | 'send'
  | 'project'
  | 'thread'
  | 'components'
  | 'part'
  | 'waiting'
  | 'loading'
  | 'failure'
  | 'success'
  | 'edit'
  | 'save'

type F026IconProps = {
  name: F026IconName
  className?: string
}

/** Small, local SVGs keep F026's chrome stable across system fonts and avoid a second icon bundle. */
export function F026Icon({ name, className }: F026IconProps) {
  const common = {
    'aria-hidden': true,
    className,
    fill: 'none',
    focusable: false,
    height: 16,
    stroke: 'currentColor',
    strokeLinecap: 'round' as const,
    strokeLinejoin: 'round' as const,
    strokeWidth: 1.8,
    viewBox: '0 0 24 24',
    width: 16,
  }
  switch (name) {
    case 'add': return <svg {...common}><path d="M12 5v14M5 12h14" /></svg>
    case 'style': return <svg {...common}><path d="M5 19 18.5 5.5a3 3 0 0 1 4.2 4.2L9.2 23.2 5 19Z" /><path d="m14.5 9.5 4 4" /></svg>
    case 'material': return <svg {...common}><rect x="4" y="4" width="16" height="16" rx="2" /><path d="M12 4v16M4 12h16" /></svg>
    case 'reference': return <svg {...common}><path d="M6 3h9l4 4v14H6z" /><path d="M15 3v5h5M9 16l2.5-3 2 2 1.5-2 2 3" /></svg>
    case 'send': return <svg {...common}><path d="m4 4 16 8-16 8 3-8-3-8Z" /><path d="M7 12h13" /></svg>
    case 'project': return <svg {...common}><path d="M3 7h7l2 2h9v10a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2Z" /><path d="M3 7V5a2 2 0 0 1 2-2h5l2 2" /></svg>
    case 'thread': return <svg {...common}><path d="M5 5h14v10H9l-4 4Z" /><path d="M8 9h8M8 12h5" /></svg>
    case 'components': return <svg {...common}><path d="m12 3 8 4.5v9L12 21l-8-4.5v-9Z" /><path d="m4 7.5 8 4.5 8-4.5M12 12v9" /></svg>
    case 'part': return <svg {...common}><path d="m12 3 7 9-7 9-7-9Z" /></svg>
    case 'waiting': return <svg {...common}><circle cx="5" cy="12" r="1" fill="currentColor" /><circle cx="12" cy="12" r="1" fill="currentColor" /><circle cx="19" cy="12" r="1" fill="currentColor" /></svg>
    case 'loading': return <svg {...common}><path d="M20 12a8 8 0 1 1-2.3-5.7" /><path d="M20 4v5h-5" /></svg>
    case 'failure': return <svg {...common}><path d="M12 8v5M12 17h.01" /><circle cx="12" cy="12" r="9" /></svg>
    case 'success': return <svg {...common}><path d="m7 12 3 3 7-7" /><circle cx="12" cy="12" r="9" /></svg>
    case 'edit': return <svg {...common}><path d="m4 20 4.2-1 10-10a2.1 2.1 0 0 0-3-3l-10 10Z" /><path d="m13.8 7.2 3 3" /></svg>
    case 'save': return <svg {...common}><path d="M5 3h12l3 3v15H5Z" /><path d="M8 3v6h8V3M8 20v-6h8v6" /></svg>
  }
}
