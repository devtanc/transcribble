import { useEffect, useState } from 'react';
import { Search, Trash2, Copy, Check } from 'lucide-react';
import { useAppStore } from '../stores/appStore';

function HistoryPage() {
  const { historyItems, loadHistory, searchHistory, deleteTranscription } =
    useAppStore();
  const [searchQuery, setSearchQuery] = useState('');
  const [copiedId, setCopiedId] = useState<number | null>(null);

  useEffect(() => {
    loadHistory();
  }, []);

  const handleSearch = (query: string) => {
    setSearchQuery(query);
    if (query.trim()) {
      searchHistory(query);
    } else {
      loadHistory();
    }
  };

  const handleCopy = async (id: number, text: string) => {
    await navigator.clipboard.writeText(text);
    setCopiedId(id);
    setTimeout(() => setCopiedId(null), 2000);
  };

  const handleDelete = async (id: number) => {
    if (confirm('Delete this transcription?')) {
      await deleteTranscription(id);
    }
  };

  return (
    <div className="p-8">
      <div className="flex items-center justify-between mb-6">
        <h2 className="text-2xl font-bold text-gray-900 dark:text-white">
          History
        </h2>

        {/* Search */}
        <div className="relative">
          <Search className="w-5 h-5 absolute left-3 top-1/2 -translate-y-1/2 text-gray-400" />
          <input
            type="text"
            placeholder="Search transcriptions..."
            value={searchQuery}
            onChange={(e) => handleSearch(e.target.value)}
            className="pl-10 pr-4 py-2 w-64 rounded-lg border border-gray-300 dark:border-gray-600 bg-white dark:bg-gray-800 text-gray-900 dark:text-white focus:ring-2 focus:ring-primary-500 focus:border-transparent"
          />
        </div>
      </div>

      {/* History List */}
      <div className="bg-white dark:bg-gray-800 rounded-xl shadow-sm border border-gray-200 dark:border-gray-700">
        {historyItems.length === 0 ? (
          <div className="p-12 text-center text-gray-500 dark:text-gray-400">
            <p className="text-lg">No transcriptions found</p>
            <p className="text-sm mt-1">
              {searchQuery
                ? 'Try a different search term'
                : 'Your transcriptions will appear here'}
            </p>
          </div>
        ) : (
          <div className="divide-y divide-gray-200 dark:divide-gray-700">
            {historyItems.map((item) => (
              <div
                key={item.id}
                className="p-4 hover:bg-gray-50 dark:hover:bg-gray-700/50 transition-colors group"
              >
                <div className="flex items-start justify-between gap-4">
                  <div className="flex-1 min-w-0">
                    <p className="text-gray-900 dark:text-white whitespace-pre-wrap">
                      {item.text}
                    </p>
                    <div className="flex items-center flex-wrap gap-4 mt-2 text-sm text-gray-500 dark:text-gray-400">
                      <span>
                        {new Date(item.timestamp).toLocaleString()}
                      </span>
                      <span>{item.word_count} words</span>
                      <span>{item.character_count} chars</span>
                      <span>{(item.duration_ms / 1000).toFixed(1)}s</span>
                      <span className="text-gray-400 dark:text-gray-500">
                        {item.model_name}
                      </span>
                    </div>
                  </div>

                  {/* Actions */}
                  <div className="flex items-center gap-2 opacity-0 group-hover:opacity-100 transition-opacity">
                    <button
                      onClick={() => handleCopy(item.id, item.text)}
                      className="p-2 rounded-lg hover:bg-gray-200 dark:hover:bg-gray-600 text-gray-500 dark:text-gray-400"
                      title="Copy to clipboard"
                    >
                      {copiedId === item.id ? (
                        <Check className="w-4 h-4 text-green-500" />
                      ) : (
                        <Copy className="w-4 h-4" />
                      )}
                    </button>
                    <button
                      onClick={() => handleDelete(item.id)}
                      className="p-2 rounded-lg hover:bg-red-100 dark:hover:bg-red-900/30 text-gray-500 hover:text-red-600 dark:text-gray-400 dark:hover:text-red-400"
                      title="Delete"
                    >
                      <Trash2 className="w-4 h-4" />
                    </button>
                  </div>
                </div>
              </div>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}

export default HistoryPage;
