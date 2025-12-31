import { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import {
  Shield,
  Mic,
  Keyboard,
  CheckCircle2,
  XCircle,
  ExternalLink,
  RefreshCw,
} from 'lucide-react';

interface PermissionStatus {
  accessibility: boolean;
  microphone: boolean;
  microphone_status: string; // "not_determined", "denied", "authorized", "restricted"
  all_granted: boolean;
}

interface PermissionsPageProps {
  onAllGranted: () => void;
}

function PermissionsPage({ onAllGranted }: PermissionsPageProps) {
  const [status, setStatus] = useState<PermissionStatus>({
    accessibility: false,
    microphone: false,
    microphone_status: 'not_determined',
    all_granted: false,
  });
  const [checking, setChecking] = useState(false);
  const [requestingMicrophone, setRequestingMicrophone] = useState(false);

  const checkPermissions = async () => {
    setChecking(true);
    try {
      const result = await invoke<PermissionStatus>('get_permission_status');
      setStatus(result);
      if (result.all_granted) {
        onAllGranted();
      }
    } catch (error) {
      console.error('Failed to check permissions:', error);
    } finally {
      setChecking(false);
    }
  };

  useEffect(() => {
    checkPermissions();
    // Check periodically in case user grants permission in System Settings
    const interval = setInterval(checkPermissions, 2000);
    return () => clearInterval(interval);
  }, []);

  const openSettings = async (pane: string) => {
    try {
      await invoke('open_permission_settings', { pane });
    } catch (error) {
      console.error('Failed to open settings:', error);
    }
  };

  const promptAccessibility = async () => {
    try {
      await invoke('prompt_accessibility_permission');
      // Check again after prompting
      setTimeout(checkPermissions, 500);
    } catch (error) {
      console.error('Failed to prompt for accessibility:', error);
    }
  };

  const promptMicrophone = async () => {
    setRequestingMicrophone(true);
    try {
      await invoke('prompt_microphone_permission');
      // Check again after prompting
      setTimeout(() => {
        checkPermissions();
        setRequestingMicrophone(false);
      }, 500);
    } catch (error) {
      console.error('Failed to prompt for microphone:', error);
      setRequestingMicrophone(false);
    }
  };

  const getMicrophoneStatusText = (micStatus: string): string => {
    switch (micStatus) {
      case 'authorized':
        return 'Granted';
      case 'denied':
        return 'Denied - Open Settings to grant access';
      case 'restricted':
        return 'Restricted by system policy';
      case 'not_determined':
      default:
        return 'Not requested yet';
    }
  };

  const permissions = [
    {
      id: 'microphone',
      name: 'Microphone',
      description: 'Required to record your voice for transcription',
      icon: Mic,
      granted: status.microphone,
      settingsPane: 'microphone',
      canPrompt: status.microphone_status === 'not_determined',
      onPrompt: promptMicrophone,
      statusText: getMicrophoneStatusText(status.microphone_status),
      isRequesting: requestingMicrophone,
    },
    {
      id: 'accessibility',
      name: 'Accessibility',
      description: 'Required to automatically type transcribed text',
      icon: Keyboard,
      granted: status.accessibility,
      settingsPane: 'accessibility',
      canPrompt: true,
      onPrompt: promptAccessibility,
      statusText: status.accessibility ? 'Granted' : 'Not granted',
      isRequesting: false,
    },
    {
      id: 'input_monitoring',
      name: 'Input Monitoring',
      description: 'Required to detect your push-to-talk hotkey globally',
      icon: Shield,
      // We can't check this programmatically, assume it needs setup if accessibility isn't granted
      granted: status.accessibility,
      settingsPane: 'input_monitoring',
      canPrompt: false,
      statusText: status.accessibility ? 'Granted (via Accessibility)' : 'Grant Accessibility first',
      isRequesting: false,
    },
  ];

  const allGranted = status.all_granted;

  return (
    <div className="min-h-screen bg-gray-50 dark:bg-gray-900 flex items-center justify-center p-8">
      <div className="max-w-xl w-full">
        <div className="text-center mb-8">
          <div className="inline-flex items-center justify-center w-16 h-16 rounded-full bg-primary-100 dark:bg-primary-900/30 mb-4">
            <Shield className="w-8 h-8 text-primary-600 dark:text-primary-400" />
          </div>
          <h1 className="text-2xl font-bold text-gray-900 dark:text-white mb-2">
            Permissions Required
          </h1>
          <p className="text-gray-600 dark:text-gray-400">
            Transcribble needs a few permissions to work properly.
            Grant access below to get started.
          </p>
        </div>

        <div className="bg-white dark:bg-gray-800 rounded-xl shadow-sm border border-gray-200 dark:border-gray-700 overflow-hidden mb-6">
          {permissions.map((permission, index) => (
            <div
              key={permission.id}
              className={`p-4 flex items-start gap-4 ${
                index !== 0 ? 'border-t border-gray-200 dark:border-gray-700' : ''
              }`}
            >
              <div
                className={`flex-shrink-0 w-10 h-10 rounded-lg flex items-center justify-center ${
                  permission.granted
                    ? 'bg-green-100 dark:bg-green-900/30'
                    : 'bg-gray-100 dark:bg-gray-700'
                }`}
              >
                <permission.icon
                  className={`w-5 h-5 ${
                    permission.granted
                      ? 'text-green-600 dark:text-green-400'
                      : 'text-gray-500 dark:text-gray-400'
                  }`}
                />
              </div>

              <div className="flex-1 min-w-0">
                <div className="flex items-center gap-2 mb-1">
                  <h3 className="font-medium text-gray-900 dark:text-white">
                    {permission.name}
                  </h3>
                  {permission.granted ? (
                    <CheckCircle2 className="w-4 h-4 text-green-500" />
                  ) : (
                    <XCircle className="w-4 h-4 text-red-500" />
                  )}
                </div>
                <p className="text-sm text-gray-500 dark:text-gray-400 mb-1">
                  {permission.description}
                </p>
                <p className={`text-xs mb-3 ${
                  permission.granted
                    ? 'text-green-600 dark:text-green-400'
                    : 'text-amber-600 dark:text-amber-400'
                }`}>
                  Status: {permission.statusText}
                </p>

                {!permission.granted && (
                  <div className="flex gap-2">
                    {permission.canPrompt && permission.onPrompt && (
                      <button
                        onClick={permission.onPrompt}
                        disabled={permission.isRequesting}
                        className="px-3 py-1.5 text-sm bg-primary-500 hover:bg-primary-600 disabled:bg-primary-400 text-white rounded-lg"
                      >
                        {permission.isRequesting ? 'Requesting...' : 'Grant Access'}
                      </button>
                    )}
                    <button
                      onClick={() => openSettings(permission.settingsPane)}
                      className="px-3 py-1.5 text-sm bg-gray-100 dark:bg-gray-700 hover:bg-gray-200 dark:hover:bg-gray-600 text-gray-700 dark:text-gray-300 rounded-lg flex items-center gap-1"
                    >
                      Open Settings
                      <ExternalLink className="w-3 h-3" />
                    </button>
                  </div>
                )}
              </div>
            </div>
          ))}
        </div>

        <div className="flex items-center justify-between">
          <button
            onClick={checkPermissions}
            disabled={checking}
            className="px-4 py-2 text-sm text-gray-600 dark:text-gray-400 hover:text-gray-900 dark:hover:text-white flex items-center gap-2"
          >
            <RefreshCw className={`w-4 h-4 ${checking ? 'animate-spin' : ''}`} />
            Check Again
          </button>

          {allGranted ? (
            <button
              onClick={onAllGranted}
              className="px-6 py-2 bg-primary-500 hover:bg-primary-600 text-white rounded-lg font-medium"
            >
              Continue to App
            </button>
          ) : (
            <p className="text-sm text-gray-500 dark:text-gray-400">
              Grant all permissions to continue
            </p>
          )}
        </div>

        <div className="text-center mt-8 p-4 bg-amber-50 dark:bg-amber-900/20 rounded-lg border border-amber-200 dark:border-amber-800">
          <p className="text-xs text-amber-700 dark:text-amber-400 mb-2">
            <strong>Warning:</strong> Skipping will limit functionality:
          </p>
          <ul className="text-xs text-amber-600 dark:text-amber-500 mb-3 list-disc list-inside">
            {!status.microphone && <li>Voice recording will not work</li>}
            {!status.accessibility && <li>Hotkey detection and auto-typing will not work</li>}
          </ul>
          <button
            onClick={onAllGranted}
            className="px-4 py-1.5 text-xs bg-amber-100 dark:bg-amber-900/40 hover:bg-amber-200 dark:hover:bg-amber-800/60 text-amber-700 dark:text-amber-400 rounded-lg border border-amber-300 dark:border-amber-700"
          >
            Skip anyway and continue
          </button>
        </div>
      </div>
    </div>
  );
}

export default PermissionsPage;
