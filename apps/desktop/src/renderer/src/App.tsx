import { useEffect, useState } from 'react'
import { Route, Routes, useLocation, useNavigate } from 'react-router'
import { AppAlerts } from './components/AppAlerts'
import { CleanupBanner } from './components/CleanupBanner'
import { DiscordExportCard } from './components/DiscordExportCard'
import { ReleaseNotes } from './components/ReleaseNotes'
import { TopNav } from './components/TopNav'
import { TooltipProvider } from './components/ui/tooltip'
import { hasDesktopApi, ipc } from './lib/ipc'
import { Account } from './routes/Account'
import { Editor } from './routes/Editor'
import { Home } from './routes/Home'
import { Library } from './routes/Library'
import { Settings } from './routes/Settings'

export default function App(): React.JSX.Element {
  const navigate = useNavigate()
  const location = useLocation()
  const [checking, setChecking] = useState(false)
  const [version, setVersion] = useState('')
  // Re-mount (and re-animate) the page when switching tabs, not when the editor swaps clips.
  const pageKey = location.pathname.startsWith('/editor') ? '/editor' : location.pathname

  useEffect(() => {
    if (!hasDesktopApi()) return
    void ipc.getVersion().then(setVersion)
    return ipc.onNavigate((path) => navigate(path))
  }, [navigate])

  return (
    <TooltipProvider>
      <div className="relative flex h-full flex-col">
        <AppAlerts />
        <ReleaseNotes />
        <DiscordExportCard />
        <TopNav />
        <CleanupBanner />
        <div key={pageKey} className="page-enter relative min-h-0 flex-1">
          <Routes location={location}>
            <Route path="/" element={<Home />} />
            <Route path="/library" element={<Library />} />
            <Route path="/editor/:clipId?" element={<Editor />} />
            <Route path="/settings" element={<Settings />} />
            <Route path="/account" element={<Account />} />
          </Routes>
        </div>
        <footer className="tc relative z-10 flex h-6 shrink-0 items-center gap-3 border-t border-border bg-sidebar px-3 text-[10px] text-muted-foreground">
          <button
            type="button"
            className="no-drag transition-colors hover:text-foreground"
            title="Check for updates"
            disabled={checking}
            onClick={() => {
              if (!hasDesktopApi() || checking) return
              setChecking(true)
              void ipc.checkForUpdates().finally(() => setChecking(false))
            }}
          >
            {checking ? 'Checking for updates…' : version ? `v${version}` : ''}
          </button>
          <span className="ml-auto">CTRL+1–5 TABS</span>
        </footer>
      </div>
    </TooltipProvider>
  )
}
