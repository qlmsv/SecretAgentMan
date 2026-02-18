'use client';

import { useState, useEffect } from 'react';
import { useRouter } from 'next/navigation';
import Link from 'next/link';
import { api } from '@/lib/api';

export default function ConnectTelegramPage() {
  const router = useRouter();
  const [link, setLink] = useState('');
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState('');
  const [checking, setChecking] = useState(false);

  useEffect(() => {
    if (!api.isAuthenticated()) {
      router.push('/login');
      return;
    }

    fetchTelegramLink();
  }, [router]);

  const fetchTelegramLink = async () => {
    try {
      const response = await api.getTelegramLink();
      setLink(response.link);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to get Telegram link');
    } finally {
      setLoading(false);
    }
  };

  const checkConnection = async () => {
    setChecking(true);
    try {
      const status = await api.getTelegramStatus();
      if (status.connected) {
        router.push('/dashboard');
      } else {
        setError('Telegram not connected yet. Please click the link and send /start to the bot.');
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to check status');
    } finally {
      setChecking(false);
    }
  };

  if (loading) {
    return (
      <main className="min-h-screen bg-gradient-to-br from-primary-900 via-primary-800 to-primary-900 flex items-center justify-center">
        <div className="text-white text-xl">Loading...</div>
      </main>
    );
  }

  return (
    <main className="min-h-screen bg-gradient-to-br from-primary-900 via-primary-800 to-primary-900 flex items-center justify-center p-4">
      <div className="card max-w-lg w-full">
        <div className="text-center mb-8">
          <div className="text-6xl mb-4">📱</div>
          <h1 className="text-2xl font-bold text-gray-900">Connect Telegram</h1>
          <p className="text-gray-600 mt-2">
            Link your Telegram account to start chatting with AI-Mentor
          </p>
        </div>

        {error && (
          <div className="bg-yellow-50 border border-yellow-200 text-yellow-700 px-4 py-3 rounded-lg mb-6">
            {error}
          </div>
        )}

        <div className="space-y-6">
          <div className="bg-gray-50 rounded-lg p-6">
            <h2 className="font-semibold text-gray-900 mb-4">How to connect:</h2>
            <ol className="space-y-3 text-gray-700">
              <li className="flex items-start gap-3">
                <span className="flex-shrink-0 w-6 h-6 bg-primary-100 text-primary-600 rounded-full flex items-center justify-center text-sm font-bold">
                  1
                </span>
                <span>Click the button below to open Telegram</span>
              </li>
              <li className="flex items-start gap-3">
                <span className="flex-shrink-0 w-6 h-6 bg-primary-100 text-primary-600 rounded-full flex items-center justify-center text-sm font-bold">
                  2
                </span>
                <span>Press "Start" or send /start to the bot</span>
              </li>
              <li className="flex items-start gap-3">
                <span className="flex-shrink-0 w-6 h-6 bg-primary-100 text-primary-600 rounded-full flex items-center justify-center text-sm font-bold">
                  3
                </span>
                <span>Come back here and click "I've Connected"</span>
              </li>
            </ol>
          </div>

          {link && (
            <a
              href={link}
              target="_blank"
              rel="noopener noreferrer"
              className="btn-primary w-full text-center flex items-center justify-center gap-2"
            >
              <svg className="w-6 h-6" viewBox="0 0 24 24" fill="currentColor">
                <path d="M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm4.64 6.8c-.15 1.58-.8 5.42-1.13 7.19-.14.75-.42 1-.68 1.03-.58.05-1.02-.38-1.58-.75-.88-.58-1.38-.94-2.23-1.5-.99-.65-.35-1.01.22-1.59.15-.15 2.71-2.48 2.76-2.69a.2.2 0 00-.05-.18c-.06-.05-.14-.03-.21-.02-.09.02-1.49.95-4.22 2.79-.4.27-.76.41-1.08.4-.36-.01-1.04-.2-1.55-.37-.63-.2-1.12-.31-1.08-.66.02-.18.27-.36.74-.55 2.92-1.27 4.86-2.11 5.83-2.51 2.78-1.16 3.35-1.36 3.73-1.36.08 0 .27.02.39.12.1.08.13.19.14.27-.01.06.01.24 0 .38z"/>
              </svg>
              Open in Telegram
            </a>
          )}

          <button
            onClick={checkConnection}
            disabled={checking}
            className="btn-secondary w-full disabled:opacity-50 disabled:cursor-not-allowed"
          >
            {checking ? 'Checking...' : "I've Connected"}
          </button>

          <div className="text-center">
            <Link href="/dashboard" className="text-gray-500 hover:text-gray-700 text-sm">
              Skip for now
            </Link>
          </div>
        </div>
      </div>
    </main>
  );
}
