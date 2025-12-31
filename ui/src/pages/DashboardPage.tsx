import { useEffect } from 'react';
import { Clock, MessageSquare, Keyboard, FileText } from 'lucide-react';
import { useAppStore } from '../stores/appStore';

function StatCard({
  icon: Icon,
  label,
  value,
  subtext,
}: {
  icon: React.ElementType;
  label: string;
  value: string | number;
  subtext?: string;
}) {
  return (
    <div className="bg-white dark:bg-gray-800 rounded-xl p-6 shadow-sm border border-gray-200 dark:border-gray-700">
      <div className="flex items-center gap-4">
        <div className="p-3 rounded-lg bg-primary-100 dark:bg-primary-900/30">
          <Icon className="w-6 h-6 text-primary-600 dark:text-primary-400" />
        </div>
        <div>
          <p className="text-sm text-gray-500 dark:text-gray-400">{label}</p>
          <p className="text-2xl font-bold text-gray-900 dark:text-white">
            {value}
          </p>
          {subtext && (
            <p className="text-xs text-gray-400 dark:text-gray-500">
              {subtext}
            </p>
          )}
        </div>
      </div>
    </div>
  );
}

function DashboardPage() {
  const { stats, historyItems, loadStats, loadHistory } = useAppStore();

  useEffect(() => {
    loadStats();
    loadHistory(5); // Load last 5 for recent transcriptions
  }, []);

  const formatDuration = (minutes: number) => {
    if (minutes < 60) {
      return `${minutes.toFixed(1)} min`;
    }
    const hours = Math.floor(minutes / 60);
    const mins = Math.round(minutes % 60);
    return `${hours}h ${mins}m`;
  };

  return (
    <div className="p-8">
      <h2 className="text-2xl font-bold text-gray-900 dark:text-white mb-6">
        Dashboard
      </h2>

      {/* Stats Grid */}
      <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4 mb-8">
        <StatCard
          icon={FileText}
          label="Transcriptions"
          value={stats?.total_transcriptions || 0}
        />
        <StatCard
          icon={MessageSquare}
          label="Words Transcribed"
          value={stats?.total_words?.toLocaleString() || 0}
        />
        <StatCard
          icon={Clock}
          label="Time Transcribed"
          value={formatDuration(stats?.total_minutes || 0)}
        />
        <StatCard
          icon={Keyboard}
          label="Keystrokes Saved"
          value={stats?.total_keystrokes_saved?.toLocaleString() || 0}
        />
      </div>

      {/* Recent Transcriptions */}
      <div className="bg-white dark:bg-gray-800 rounded-xl shadow-sm border border-gray-200 dark:border-gray-700">
        <div className="p-4 border-b border-gray-200 dark:border-gray-700">
          <h3 className="text-lg font-semibold text-gray-900 dark:text-white">
            Recent Transcriptions
          </h3>
        </div>
        <div className="divide-y divide-gray-200 dark:divide-gray-700">
          {historyItems.length === 0 ? (
            <div className="p-8 text-center text-gray-500 dark:text-gray-400">
              <MessageSquare className="w-12 h-12 mx-auto mb-4 opacity-50" />
              <p>No transcriptions yet</p>
              <p className="text-sm mt-1">
                Hold your hotkey and speak to create your first transcription
              </p>
            </div>
          ) : (
            historyItems.map((item) => (
              <div
                key={item.id}
                className="p-4 hover:bg-gray-50 dark:hover:bg-gray-700/50 transition-colors"
              >
                <p className="text-gray-900 dark:text-white line-clamp-2">
                  {item.text}
                </p>
                <div className="flex items-center gap-4 mt-2 text-sm text-gray-500 dark:text-gray-400">
                  <span>{new Date(item.timestamp).toLocaleString()}</span>
                  <span>{item.word_count} words</span>
                  <span>{(item.duration_ms / 1000).toFixed(1)}s</span>
                </div>
              </div>
            ))
          )}
        </div>
      </div>
    </div>
  );
}

export default DashboardPage;
