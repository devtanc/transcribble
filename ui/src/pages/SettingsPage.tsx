import { useState, useEffect, useRef } from 'react';
import { invoke } from '@tauri-apps/api/core';
import {
  Sun,
  Moon,
  Monitor,
  Download,
  Check,
  Keyboard,
  Trash2,
  Shield,
  CheckCircle,
} from 'lucide-react';
import { useAppStore } from '../stores/appStore';

interface SettingsPageProps {
  onOpenPermissions?: () => void;
}

function SettingsPage({ onOpenPermissions }: SettingsPageProps) {
  const {
    config,
    models,
    theme,
    downloadingModel,
    downloadProgress,
    loadConfig,
    loadModels,
    saveConfig,
    downloadModel,
    setActiveModel,
    setTheme,
    clearHistory,
  } = useAppStore();

  const [hotkey, setHotkey] = useState('');
  const [autoType, setAutoType] = useState(true);
  const [isRecordingHotkey, setIsRecordingHotkey] = useState(false);
  const [hasChanges, setHasChanges] = useState(false);

  // Hotkey testing state
  const [isHotkeyPressed, setIsHotkeyPressed] = useState(false);
  const [transcriptionStatus, setTranscriptionStatus] = useState<{
    type: 'idle' | 'recording' | 'processing' | 'success' | 'error';
    text?: string;
  }>({ type: 'idle' });
  const [testInputValue, setTestInputValue] = useState('');
  const testInputRef = useRef<HTMLInputElement>(null);
  const clearTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => {
    loadConfig();
    loadModels();
  }, []);

  useEffect(() => {
    if (config) {
      setHotkey(config.hotkey);
      setAutoType(config.auto_type);
    }
  }, [config]);

  // Enable test mode on mount, disable on unmount
  useEffect(() => {
    invoke('set_test_mode', { enabled: true });

    return () => {
      invoke('set_test_mode', { enabled: false });
    };
  }, []);

  // Listen for recording and transcription events
  useEffect(() => {
    const handleRecordingStarted = () => {
      setIsHotkeyPressed(true);
      setTranscriptionStatus({ type: 'recording' });
      // Focus the test input
      testInputRef.current?.focus();
    };

    const handleRecordingStopped = () => {
      setIsHotkeyPressed(false);
      setTranscriptionStatus({ type: 'processing' });

      // Fallback timeout - if no result after 30 seconds, show error
      if (clearTimeoutRef.current) {
        clearTimeout(clearTimeoutRef.current);
      }
      clearTimeoutRef.current = setTimeout(() => {
        setTranscriptionStatus({ type: 'error', text: 'Transcription timed out' });
        setTimeout(() => {
          setTranscriptionStatus({ type: 'idle' });
        }, 3000);
      }, 30000);
    };

    const handleTranscriptionResult = (
      event: CustomEvent<{
        text: string;
        duration_ms: number;
        word_count: number;
      }>
    ) => {
      const { text } = event.detail;
      setTranscriptionStatus({ type: 'success', text });

      // Note: The auto-type feature will type into the focused input
      // We wait a brief moment then update state to reflect this
      setTimeout(() => {
        setTestInputValue(text);
      }, 150);

      // Clear after 1 second
      if (clearTimeoutRef.current) {
        clearTimeout(clearTimeoutRef.current);
      }
      clearTimeoutRef.current = setTimeout(() => {
        setTranscriptionStatus({ type: 'idle' });
        setTestInputValue('');
      }, 1000);
    };

    const handleTranscriptionError = (event: CustomEvent<string>) => {
      setTranscriptionStatus({ type: 'error', text: event.detail });

      // Clear error after 3 seconds
      if (clearTimeoutRef.current) {
        clearTimeout(clearTimeoutRef.current);
      }
      clearTimeoutRef.current = setTimeout(() => {
        setTranscriptionStatus({ type: 'idle' });
      }, 3000);
    };

    // Subscribe to custom events from App.tsx
    window.addEventListener(
      'recording-started',
      handleRecordingStarted as EventListener
    );
    window.addEventListener(
      'recording-stopped',
      handleRecordingStopped as EventListener
    );
    window.addEventListener(
      'transcription-result',
      handleTranscriptionResult as EventListener
    );
    window.addEventListener(
      'transcription-error',
      handleTranscriptionError as EventListener
    );

    return () => {
      window.removeEventListener(
        'recording-started',
        handleRecordingStarted as EventListener
      );
      window.removeEventListener(
        'recording-stopped',
        handleRecordingStopped as EventListener
      );
      window.removeEventListener(
        'transcription-result',
        handleTranscriptionResult as EventListener
      );
      window.removeEventListener(
        'transcription-error',
        handleTranscriptionError as EventListener
      );

      if (clearTimeoutRef.current) {
        clearTimeout(clearTimeoutRef.current);
      }
    };
  }, []);

  const handleHotkeyRecord = () => {
    setIsRecordingHotkey(true);

    const handleKeyDown = (e: KeyboardEvent) => {
      e.preventDefault();

      // Map key to our hotkey format
      let keyName = '';
      if (e.key === 'Alt') {
        keyName = e.location === 2 ? 'RightAlt' : 'LeftAlt';
      } else if (e.key === 'Control') {
        keyName = e.location === 2 ? 'RightControl' : 'LeftControl';
      } else if (e.key === 'Shift') {
        keyName = e.location === 2 ? 'RightShift' : 'LeftShift';
      } else if (e.key.startsWith('F') && e.key.length <= 3) {
        keyName = e.key;
      } else {
        // Unsupported key
        return;
      }

      setHotkey(keyName);
      setHasChanges(true);
      setIsRecordingHotkey(false);
      window.removeEventListener('keydown', handleKeyDown);
    };

    window.addEventListener('keydown', handleKeyDown);
  };

  const handleSave = async () => {
    await saveConfig(hotkey, autoType);
    setHasChanges(false);
  };

  const handleDownloadModel = async (name: string) => {
    try {
      await downloadModel(name);
    } catch (error) {
      console.error('Failed to download model:', error);
    }
  };

  const handleSetActiveModel = async (name: string) => {
    try {
      await setActiveModel(name);
    } catch (error) {
      console.error('Failed to set active model:', error);
    }
  };

  const handleClearHistory = async () => {
    if (confirm('Are you sure you want to clear all transcription history?')) {
      await clearHistory();
    }
  };

  const themeOptions = [
    { value: 'light', label: 'Light', icon: Sun },
    { value: 'dark', label: 'Dark', icon: Moon },
    { value: 'system', label: 'System', icon: Monitor },
  ] as const;

  return (
    <div className="p-8 max-w-3xl">
      <h2 className="text-2xl font-bold text-gray-900 dark:text-white mb-6">
        Settings
      </h2>

      {/* Hotkey Section */}
      <section className="mb-8">
        <h3 className="text-lg font-semibold text-gray-900 dark:text-white mb-4">
          Push-to-Talk Hotkey
        </h3>
        <div className="bg-white dark:bg-gray-800 rounded-xl p-6 shadow-sm border border-gray-200 dark:border-gray-700 relative">
          {/* Recording indicator dot - top right corner */}
          <div className="absolute top-4 right-4 flex items-center gap-2">
            <span className="text-xs text-gray-500 dark:text-gray-400">
              {isHotkeyPressed ? 'Recording' : 'Ready'}
            </span>
            <div
              className={`w-3 h-3 rounded-full transition-colors ${
                isHotkeyPressed
                  ? 'bg-red-500 animate-pulse'
                  : 'bg-gray-300 dark:bg-gray-600'
              }`}
            />
          </div>

          <div className="flex items-center gap-4">
            <div className="flex-1">
              <p className="text-sm text-gray-500 dark:text-gray-400 mb-2">
                Hold this key while speaking to record
              </p>
              <div className="flex items-center gap-3">
                <div className="px-4 py-2 bg-gray-100 dark:bg-gray-700 rounded-lg font-mono text-gray-900 dark:text-white">
                  {hotkey || 'Not set'}
                </div>
                <button
                  onClick={handleHotkeyRecord}
                  className={`px-4 py-2 rounded-lg flex items-center gap-2 ${
                    isRecordingHotkey
                      ? 'bg-red-500 text-white animate-pulse'
                      : 'bg-primary-500 hover:bg-primary-600 text-white'
                  }`}
                >
                  <Keyboard className="w-4 h-4" />
                  {isRecordingHotkey ? 'Press a key...' : 'Record Key'}
                </button>
              </div>
            </div>
          </div>

          {/* Test Hotkey Section */}
          <div className="mt-4 pt-4 border-t border-gray-200 dark:border-gray-700">
            <p className="text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
              Test Your Hotkey
            </p>
            <p className="text-xs text-gray-500 dark:text-gray-400 mb-3">
              Press and hold your hotkey, speak, then release. The transcribed
              text will appear below.
            </p>

            {/* Test Input */}
            <input
              ref={testInputRef}
              type="text"
              value={testInputValue}
              onChange={(e) => setTestInputValue(e.target.value)}
              autoFocus
              placeholder="Transcribed text will appear here..."
              className={`w-full px-4 py-3 rounded-lg border-2 transition-colors ${
                transcriptionStatus.type === 'success'
                  ? 'border-green-500 bg-green-50 dark:bg-green-900/20'
                  : transcriptionStatus.type === 'error'
                    ? 'border-red-500 bg-red-50 dark:bg-red-900/20'
                    : transcriptionStatus.type === 'recording'
                      ? 'border-red-500 animate-pulse'
                      : 'border-gray-200 dark:border-gray-600 bg-gray-50 dark:bg-gray-700'
              } text-gray-900 dark:text-white focus:outline-none focus:ring-2 focus:ring-primary-500`}
            />

            {/* Status Line */}
            <div className="mt-2 h-6 flex items-center gap-2">
              {transcriptionStatus.type === 'recording' && (
                <>
                  <div className="w-2 h-2 bg-red-500 rounded-full animate-pulse" />
                  <span className="text-sm text-red-500">Recording...</span>
                </>
              )}
              {transcriptionStatus.type === 'processing' && (
                <>
                  <div className="w-4 h-4 border-2 border-primary-500 border-t-transparent rounded-full animate-spin" />
                  <span className="text-sm text-primary-500">Processing...</span>
                </>
              )}
              {transcriptionStatus.type === 'success' && (
                <>
                  <CheckCircle className="w-4 h-4 text-green-500" />
                  <span className="text-sm text-green-500">
                    Transcription successful! (
                    {transcriptionStatus.text?.split(' ').length || 0} words)
                  </span>
                </>
              )}
              {transcriptionStatus.type === 'error' && (
                <span className="text-sm text-red-500">
                  Error: {transcriptionStatus.text}
                </span>
              )}
            </div>
          </div>

          {/* Auto-type toggle */}
          <div className="mt-4 pt-4 border-t border-gray-200 dark:border-gray-700">
            <label className="flex items-center justify-between cursor-pointer">
              <div>
                <p className="font-medium text-gray-900 dark:text-white">
                  Auto-type transcription
                </p>
                <p className="text-sm text-gray-500 dark:text-gray-400">
                  Automatically type the transcribed text after recording
                </p>
              </div>
              <div
                className={`w-12 h-6 rounded-full p-1 transition-colors ${
                  autoType ? 'bg-primary-500' : 'bg-gray-300 dark:bg-gray-600'
                }`}
                onClick={() => {
                  setAutoType(!autoType);
                  setHasChanges(true);
                }}
              >
                <div
                  className={`w-4 h-4 rounded-full bg-white transition-transform ${
                    autoType ? 'translate-x-6' : ''
                  }`}
                />
              </div>
            </label>
          </div>

          {hasChanges && (
            <div className="mt-4 pt-4 border-t border-gray-200 dark:border-gray-700">
              <button
                onClick={handleSave}
                className="px-4 py-2 bg-primary-500 hover:bg-primary-600 text-white rounded-lg"
              >
                Save Changes
              </button>
            </div>
          )}
        </div>
      </section>

      {/* Model Section */}
      <section className="mb-8">
        <h3 className="text-lg font-semibold text-gray-900 dark:text-white mb-4">
          Whisper Model
        </h3>
        <div className="bg-white dark:bg-gray-800 rounded-xl shadow-sm border border-gray-200 dark:border-gray-700 overflow-hidden">
          <div className="divide-y divide-gray-200 dark:divide-gray-700">
            {models.map((model) => (
              <div
                key={model.name}
                className="p-4 flex items-center justify-between"
              >
                <div className="flex-1">
                  <div className="flex items-center gap-2">
                    <span className="font-medium text-gray-900 dark:text-white">
                      {model.name}
                    </span>
                    {model.active && (
                      <span className="px-2 py-0.5 text-xs bg-primary-100 dark:bg-primary-900/30 text-primary-700 dark:text-primary-300 rounded">
                        Active
                      </span>
                    )}
                    {model.english_only && (
                      <span className="px-2 py-0.5 text-xs bg-gray-100 dark:bg-gray-700 text-gray-600 dark:text-gray-400 rounded">
                        English
                      </span>
                    )}
                  </div>
                  <p className="text-sm text-gray-500 dark:text-gray-400">
                    {model.description} ({model.size_mb} MB)
                  </p>
                </div>

                <div className="flex items-center gap-2">
                  {model.downloaded ? (
                    <button
                      onClick={() => handleSetActiveModel(model.name)}
                      disabled={model.active}
                      className={`px-3 py-1.5 rounded-lg text-sm flex items-center gap-1 ${
                        model.active
                          ? 'bg-green-100 dark:bg-green-900/30 text-green-700 dark:text-green-300'
                          : 'bg-gray-100 dark:bg-gray-700 hover:bg-gray-200 dark:hover:bg-gray-600 text-gray-700 dark:text-gray-300'
                      }`}
                    >
                      {model.active ? (
                        <>
                          <Check className="w-4 h-4" />
                          Active
                        </>
                      ) : (
                        'Use'
                      )}
                    </button>
                  ) : (
                    <button
                      onClick={() => handleDownloadModel(model.name)}
                      disabled={downloadingModel === model.name}
                      className="px-3 py-1.5 bg-primary-500 hover:bg-primary-600 disabled:bg-primary-400 text-white rounded-lg text-sm flex items-center gap-1"
                    >
                      {downloadingModel === model.name ? (
                        <>
                          <div className="w-4 h-4 border-2 border-white border-t-transparent rounded-full animate-spin" />
                          {downloadProgress.toFixed(0)}%
                        </>
                      ) : (
                        <>
                          <Download className="w-4 h-4" />
                          Download
                        </>
                      )}
                    </button>
                  )}
                </div>
              </div>
            ))}
          </div>
        </div>
      </section>

      {/* Theme Section */}
      <section className="mb-8">
        <h3 className="text-lg font-semibold text-gray-900 dark:text-white mb-4">
          Appearance
        </h3>
        <div className="bg-white dark:bg-gray-800 rounded-xl p-6 shadow-sm border border-gray-200 dark:border-gray-700">
          <p className="text-sm text-gray-500 dark:text-gray-400 mb-4">
            Choose your preferred theme
          </p>
          <div className="flex gap-2">
            {themeOptions.map(({ value, label, icon: Icon }) => (
              <button
                key={value}
                onClick={() => setTheme(value)}
                className={`flex-1 px-4 py-3 rounded-lg flex items-center justify-center gap-2 transition-colors ${
                  theme === value
                    ? 'bg-primary-100 dark:bg-primary-900/30 text-primary-700 dark:text-primary-300 border-2 border-primary-500'
                    : 'bg-gray-100 dark:bg-gray-700 text-gray-700 dark:text-gray-300 border-2 border-transparent hover:bg-gray-200 dark:hover:bg-gray-600'
                }`}
              >
                <Icon className="w-5 h-5" />
                {label}
              </button>
            ))}
          </div>
        </div>
      </section>

      {/* Permissions Section */}
      <section className="mb-8">
        <h3 className="text-lg font-semibold text-gray-900 dark:text-white mb-4">
          Permissions
        </h3>
        <div className="bg-white dark:bg-gray-800 rounded-xl p-6 shadow-sm border border-gray-200 dark:border-gray-700">
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-4">
              <div className="w-10 h-10 rounded-lg bg-primary-100 dark:bg-primary-900/30 flex items-center justify-center">
                <Shield className="w-5 h-5 text-primary-600 dark:text-primary-400" />
              </div>
              <div>
                <p className="font-medium text-gray-900 dark:text-white">
                  System Permissions
                </p>
                <p className="text-sm text-gray-500 dark:text-gray-400">
                  Manage microphone, accessibility, and input monitoring permissions
                </p>
              </div>
            </div>
            <button
              onClick={onOpenPermissions}
              className="px-4 py-2 bg-gray-100 dark:bg-gray-700 hover:bg-gray-200 dark:hover:bg-gray-600 text-gray-700 dark:text-gray-300 rounded-lg"
            >
              Review Permissions
            </button>
          </div>
        </div>
      </section>

      {/* Danger Zone */}
      <section>
        <h3 className="text-lg font-semibold text-red-600 dark:text-red-400 mb-4">
          Danger Zone
        </h3>
        <div className="bg-white dark:bg-gray-800 rounded-xl p-6 shadow-sm border border-red-200 dark:border-red-900">
          <div className="flex items-center justify-between">
            <div>
              <p className="font-medium text-gray-900 dark:text-white">
                Clear History
              </p>
              <p className="text-sm text-gray-500 dark:text-gray-400">
                Permanently delete all transcription history
              </p>
            </div>
            <button
              onClick={handleClearHistory}
              className="px-4 py-2 bg-red-500 hover:bg-red-600 text-white rounded-lg flex items-center gap-2"
            >
              <Trash2 className="w-4 h-4" />
              Clear All
            </button>
          </div>
        </div>
      </section>
    </div>
  );
}

export default SettingsPage;
