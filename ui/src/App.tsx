import { useEffect, useState } from 'react';
import { Routes, Route, NavLink } from 'react-router-dom';
import { listen } from '@tauri-apps/api/event';
import { invoke } from '@tauri-apps/api/core';
import {
  Home,
  History,
  Settings,
  Mic,
  MicOff,
  Download,
} from 'lucide-react';
import { useAppStore } from './stores/appStore';
import DashboardPage from './pages/DashboardPage';
import HistoryPage from './pages/HistoryPage';
import SettingsPage from './pages/SettingsPage';
import PermissionsPage from './pages/PermissionsPage';

interface PermissionStatus {
  accessibility: boolean;
  microphone: boolean;
  microphone_status: string;
  all_granted: boolean;
}

interface PermissionError {
  permission: string;
  message: string;
}

interface TranscriptionResult {
  text: string;
  duration_ms: number;
  word_count: number;
}

function App() {
  const {
    isRecording,
    downloadingModel,
    downloadProgress,
    activeModel,
    theme,
    loadConfig,
    loadModels,
    loadStats,
    loadTheme,
  } = useAppStore();

  const [showPermissions, setShowPermissions] = useState<boolean | null>(null);

  // Check permissions on mount
  useEffect(() => {
    const checkPermissions = async () => {
      try {
        const status = await invoke<PermissionStatus>('get_permission_status');
        if (status.all_granted) {
          // Permissions already granted, start the listener
          await invoke('start_listener');
          setShowPermissions(false);
        } else {
          setShowPermissions(true);
        }
      } catch {
        // If we can't check, assume we need to show the page
        setShowPermissions(true);
      }
    };
    checkPermissions();
  }, []);

  // Initialize app
  useEffect(() => {
    loadConfig();
    loadModels();
    loadStats();
    loadTheme();

    // Listen for Tauri events
    const unlistenDownload = listen<{
      model_name: string;
      percent: number;
    }>('download-progress', (event) => {
      useAppStore.setState({
        downloadProgress: event.payload.percent,
      });
    });

    const unlistenComplete = listen<string>('download-complete', () => {
      loadModels();
    });

    // Listen for permission errors from the backend
    const unlistenPermissionError = listen<PermissionError>('permission-error', (event) => {
      console.warn('Permission error:', event.payload);
      // Show permissions page when there's a permission error
      setShowPermissions(true);
    });

    // Listen for recording state changes
    const unlistenRecordingStarted = listen('recording-started', () => {
      useAppStore.setState({ isRecording: true });
      window.dispatchEvent(new CustomEvent('recording-started'));
    });

    const unlistenRecordingStopped = listen('recording-stopped', () => {
      useAppStore.setState({ isRecording: false });
      window.dispatchEvent(new CustomEvent('recording-stopped'));
    });

    // Listen for transcription events
    const unlistenTranscriptionComplete = listen<TranscriptionResult>(
      'transcription-complete',
      (event) => {
        window.dispatchEvent(
          new CustomEvent('transcription-result', { detail: event.payload })
        );
      }
    );

    const unlistenTranscriptionError = listen<string>(
      'transcription-error',
      (event) => {
        window.dispatchEvent(
          new CustomEvent('transcription-error', { detail: event.payload })
        );
      }
    );

    // Listen for listener status events
    const unlistenListenerStarted = listen<{ hotkey: string; keycode: number }>(
      'listener-started',
      (event) => {
        console.log('Hotkey listener started:', event.payload.hotkey);
        window.dispatchEvent(
          new CustomEvent('listener-started', { detail: event.payload })
        );
      }
    );

    const unlistenListenerError = listen<{ error: string }>(
      'listener-error',
      (event) => {
        console.error('Hotkey listener error:', event.payload.error);
        window.dispatchEvent(
          new CustomEvent('listener-error', { detail: event.payload })
        );
      }
    );

    return () => {
      unlistenDownload.then((fn) => fn());
      unlistenComplete.then((fn) => fn());
      unlistenPermissionError.then((fn) => fn());
      unlistenRecordingStarted.then((fn) => fn());
      unlistenRecordingStopped.then((fn) => fn());
      unlistenTranscriptionComplete.then((fn) => fn());
      unlistenTranscriptionError.then((fn) => fn());
      unlistenListenerStarted.then((fn) => fn());
      unlistenListenerError.then((fn) => fn());
    };
  }, []);

  // Listen for system theme changes
  useEffect(() => {
    if (theme === 'system') {
      const mediaQuery = window.matchMedia('(prefers-color-scheme: dark)');
      const handler = (e: MediaQueryListEvent) => {
        document.documentElement.classList.toggle('dark', e.matches);
        document.documentElement.classList.toggle('light', !e.matches);
      };
      mediaQuery.addEventListener('change', handler);
      return () => mediaQuery.removeEventListener('change', handler);
    }
  }, [theme]);

  const navItems = [
    { path: '/', icon: Home, label: 'Dashboard' },
    { path: '/history', icon: History, label: 'History' },
    { path: '/settings', icon: Settings, label: 'Settings' },
  ];

  // Show loading state while checking permissions
  if (showPermissions === null) {
    return (
      <div className="h-screen bg-gray-100 dark:bg-gray-900 flex items-center justify-center">
        <div className="text-gray-500">Loading...</div>
      </div>
    );
  }

  // Show permissions page if needed
  if (showPermissions) {
    return (
      <PermissionsPage
        onAllGranted={async () => {
          // Start the hotkey listener now that permissions are granted
          try {
            await invoke('start_listener');
          } catch (error) {
            console.error('Failed to start listener:', error);
          }
          setShowPermissions(false);
        }}
      />
    );
  }

  return (
    <div className="flex h-screen bg-gray-100 dark:bg-gray-900">
      {/* Sidebar */}
      <aside className="w-64 bg-white dark:bg-gray-800 border-r border-gray-200 dark:border-gray-700 flex flex-col">
        {/* Logo */}
        <div className="p-4 border-b border-gray-200 dark:border-gray-700">
          <h1 className="text-xl font-bold text-gray-900 dark:text-white flex items-center gap-2">
            {isRecording ? (
              <Mic className="w-6 h-6 text-red-500 animate-pulse" />
            ) : (
              <MicOff className="w-6 h-6 text-gray-400" />
            )}
            Transcribble
          </h1>
          <p className="text-sm text-gray-500 dark:text-gray-400 mt-1">
            {activeModel || 'No model loaded'}
          </p>
        </div>

        {/* Navigation */}
        <nav className="flex-1 p-4 space-y-1">
          {navItems.map(({ path, icon: Icon, label }) => (
            <NavLink
              key={path}
              to={path}
              className={({ isActive }) =>
                `flex items-center gap-3 px-3 py-2 rounded-lg transition-colors ${
                  isActive
                    ? 'bg-primary-100 dark:bg-primary-900/30 text-primary-700 dark:text-primary-300'
                    : 'text-gray-600 dark:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-700'
                }`
              }
            >
              <Icon className="w-5 h-5" />
              {label}
            </NavLink>
          ))}
        </nav>

        {/* Download Progress */}
        {downloadingModel && (
          <div className="p-4 border-t border-gray-200 dark:border-gray-700">
            <div className="flex items-center gap-2 text-sm text-gray-600 dark:text-gray-300">
              <Download className="w-4 h-4 animate-bounce" />
              <span>Downloading {downloadingModel}</span>
            </div>
            <div className="mt-2 w-full bg-gray-200 dark:bg-gray-700 rounded-full h-2">
              <div
                className="bg-primary-500 h-2 rounded-full transition-all"
                style={{ width: `${downloadProgress}%` }}
              />
            </div>
            <p className="text-xs text-gray-500 mt-1">
              {downloadProgress.toFixed(1)}%
            </p>
          </div>
        )}

        {/* Status Bar */}
        <div className="p-4 border-t border-gray-200 dark:border-gray-700">
          <div className="flex items-center justify-between text-sm">
            <span className="text-gray-500 dark:text-gray-400">Status</span>
            <span
              className={`flex items-center gap-1 ${
                isRecording
                  ? 'text-red-500'
                  : 'text-green-500'
              }`}
            >
              <span
                className={`w-2 h-2 rounded-full ${
                  isRecording
                    ? 'bg-red-500 animate-pulse'
                    : 'bg-green-500'
                }`}
              />
              {isRecording ? 'Recording' : 'Ready'}
            </span>
          </div>
        </div>
      </aside>

      {/* Main Content */}
      <main className="flex-1 overflow-auto">
        <Routes>
          <Route path="/" element={<DashboardPage />} />
          <Route path="/history" element={<HistoryPage />} />
          <Route path="/settings" element={<SettingsPage onOpenPermissions={() => setShowPermissions(true)} />} />
        </Routes>
      </main>
    </div>
  );
}

export default App;
