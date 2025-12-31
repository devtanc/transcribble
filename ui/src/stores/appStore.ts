import { create } from 'zustand';
import { invoke } from '@tauri-apps/api/core';

// Types
export interface ModelInfo {
  name: string;
  filename: string;
  size_mb: number;
  description: string;
  english_only: boolean;
  downloaded: boolean;
  active: boolean;
}

export interface TranscriptionRecord {
  id: number;
  timestamp: string;
  text: string;
  duration_ms: number;
  word_count: number;
  character_count: number;
  keystrokes_saved: number;
  model_name: string;
  sample_rate: number | null;
  audio_device: string | null;
  processing_time_ms: number | null;
  created_at: string;
}

export interface Statistics {
  total_transcriptions: number;
  total_words: number;
  total_duration_ms: number;
  total_keystrokes_saved: number;
  total_minutes: number;
}

export interface Config {
  model_name: string;
  model_path: string;
  hotkey: string;
  auto_type: boolean;
  show_word_count: boolean;
  show_duration: boolean;
  history_enabled: boolean;
}

interface AppState {
  // Recording state
  isListening: boolean;
  isRecording: boolean;

  // Model state
  models: ModelInfo[];
  activeModel: string | null;
  downloadingModel: string | null;
  downloadProgress: number;

  // Settings
  config: Config | null;
  theme: 'light' | 'dark' | 'system';

  // History
  historyItems: TranscriptionRecord[];
  historyTotal: number;

  // Statistics
  stats: Statistics | null;

  // Actions
  loadConfig: () => Promise<void>;
  saveConfig: (hotkey: string, autoType: boolean) => Promise<void>;
  loadModels: () => Promise<void>;
  downloadModel: (name: string) => Promise<void>;
  setActiveModel: (name: string) => Promise<void>;
  loadHistory: (limit?: number, offset?: number) => Promise<void>;
  loadStats: () => Promise<void>;
  searchHistory: (query: string) => Promise<void>;
  deleteTranscription: (id: number) => Promise<void>;
  clearHistory: () => Promise<void>;
  setTheme: (theme: 'light' | 'dark' | 'system') => Promise<void>;
  loadTheme: () => Promise<void>;
}

export const useAppStore = create<AppState>((set, get) => ({
  // Initial state
  isListening: false,
  isRecording: false,
  models: [],
  activeModel: null,
  downloadingModel: null,
  downloadProgress: 0,
  config: null,
  theme: 'system',
  historyItems: [],
  historyTotal: 0,
  stats: null,

  // Actions
  loadConfig: async () => {
    try {
      const config = await invoke<Config>('get_config');
      set({ config, activeModel: config.model_name });
    } catch (error) {
      console.error('Failed to load config:', error);
    }
  },

  saveConfig: async (hotkey: string, autoType: boolean) => {
    try {
      await invoke('save_config', {
        hotkey,
        autoType,
        showWordCount: true,
        showDuration: true,
      });
      await get().loadConfig();
    } catch (error) {
      console.error('Failed to save config:', error);
      throw error;
    }
  },

  loadModels: async () => {
    try {
      const models = await invoke<ModelInfo[]>('get_available_models');
      const activeModel = models.find((m) => m.active)?.name || null;
      set({ models, activeModel });
    } catch (error) {
      console.error('Failed to load models:', error);
    }
  },

  downloadModel: async (name: string) => {
    set({ downloadingModel: name, downloadProgress: 0 });
    try {
      await invoke('download_model', { modelName: name });
      await get().loadModels();
    } catch (error) {
      console.error('Failed to download model:', error);
      throw error;
    } finally {
      set({ downloadingModel: null, downloadProgress: 0 });
    }
  },

  setActiveModel: async (name: string) => {
    try {
      await invoke('set_active_model', { modelName: name });
      set({ activeModel: name });
      await get().loadModels();
    } catch (error) {
      console.error('Failed to set active model:', error);
      throw error;
    }
  },

  loadHistory: async (limit = 50, offset = 0) => {
    try {
      const items = await invoke<TranscriptionRecord[]>('get_history', {
        limit,
        offset,
      });
      set({ historyItems: items });
    } catch (error) {
      console.error('Failed to load history:', error);
    }
  },

  loadStats: async () => {
    try {
      const stats = await invoke<Statistics>('get_statistics');
      set({ stats });
    } catch (error) {
      console.error('Failed to load stats:', error);
    }
  },

  searchHistory: async (query: string) => {
    try {
      const items = await invoke<TranscriptionRecord[]>('search_history', {
        query,
        limit: 50,
      });
      set({ historyItems: items });
    } catch (error) {
      console.error('Failed to search history:', error);
    }
  },

  deleteTranscription: async (id: number) => {
    try {
      await invoke('delete_transcription', { id });
      await get().loadHistory();
      await get().loadStats();
    } catch (error) {
      console.error('Failed to delete transcription:', error);
      throw error;
    }
  },

  clearHistory: async () => {
    try {
      await invoke('clear_history');
      set({ historyItems: [] });
      await get().loadStats();
    } catch (error) {
      console.error('Failed to clear history:', error);
      throw error;
    }
  },

  setTheme: async (theme: 'light' | 'dark' | 'system') => {
    try {
      await invoke('set_theme', { theme });
      set({ theme });

      // Apply theme to document
      const root = document.documentElement;
      if (theme === 'system') {
        const prefersDark = window.matchMedia(
          '(prefers-color-scheme: dark)'
        ).matches;
        root.classList.toggle('dark', prefersDark);
        root.classList.toggle('light', !prefersDark);
      } else {
        root.classList.toggle('dark', theme === 'dark');
        root.classList.toggle('light', theme === 'light');
      }
    } catch (error) {
      console.error('Failed to set theme:', error);
    }
  },

  loadTheme: async () => {
    try {
      const theme = (await invoke<string>('get_theme')) as
        | 'light'
        | 'dark'
        | 'system';
      set({ theme });

      // Apply theme to document
      const root = document.documentElement;
      if (theme === 'system') {
        const prefersDark = window.matchMedia(
          '(prefers-color-scheme: dark)'
        ).matches;
        root.classList.toggle('dark', prefersDark);
        root.classList.toggle('light', !prefersDark);
      } else {
        root.classList.toggle('dark', theme === 'dark');
        root.classList.toggle('light', theme === 'light');
      }
    } catch (error) {
      console.error('Failed to load theme:', error);
    }
  },
}));
