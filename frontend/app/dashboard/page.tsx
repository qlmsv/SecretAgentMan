'use client';

import { useState, useEffect } from 'react';
import { useRouter } from 'next/navigation';
import Link from 'next/link';
import { api, UsageResponse, TokenPackage } from '@/lib/api';

export default function DashboardPage() {
  const router = useRouter();
  const [usage, setUsage] = useState<UsageResponse | null>(null);
  const [packages, setPackages] = useState<TokenPackage[]>([]);
  const [telegramConnected, setTelegramConnected] = useState(false);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState('');

  useEffect(() => {
    if (!api.isAuthenticated()) {
      router.push('/login');
      return;
    }

    loadData();
  }, [router]);

  const loadData = async () => {
    try {
      const [usageData, packagesData, telegramStatus] = await Promise.all([
        api.getUsage(),
        api.getPackages(),
        api.getTelegramStatus(),
      ]);
      setUsage(usageData);
      setPackages(packagesData);
      setTelegramConnected(telegramStatus.connected);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to load data');
    } finally {
      setLoading(false);
    }
  };

  const handleBuyTokens = async (packageId: string) => {
    try {
      const response = await api.createPayment(packageId);
      window.location.href = response.payment_url;
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to create payment');
    }
  };

  const handleLogout = () => {
    api.logout();
    router.push('/');
  };

  if (loading) {
    return (
      <main className="min-h-screen bg-gray-50 flex items-center justify-center">
        <div className="text-gray-600 text-xl">Loading...</div>
      </main>
    );
  }

  const usagePercent = usage
    ? Math.round((usage.trial_tokens_used / usage.trial_tokens_limit) * 100)
    : 0;

  return (
    <main className="min-h-screen bg-gray-50">
      {/* Header */}
      <header className="bg-white shadow-sm">
        <div className="container mx-auto px-4 py-4 flex justify-between items-center">
          <Link href="/" className="text-xl font-bold text-primary-600">
            AI-Mentor
          </Link>
          <button
            onClick={handleLogout}
            className="text-gray-600 hover:text-gray-800"
          >
            Logout
          </button>
        </div>
      </header>

      <div className="container mx-auto px-4 py-8">
        {error && (
          <div className="bg-red-50 border border-red-200 text-red-700 px-4 py-3 rounded-lg mb-6">
            {error}
          </div>
        )}

        <div className="grid md:grid-cols-2 gap-6 mb-8">
          {/* Telegram Status */}
          <div className="card">
            <h2 className="text-lg font-semibold text-gray-900 mb-4">Telegram Connection</h2>
            {telegramConnected ? (
              <div className="flex items-center gap-3 text-green-600">
                <svg className="w-6 h-6" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 12l2 2 4-4m6 2a9 9 0 11-18 0 9 9 0 0118 0z" />
                </svg>
                <span className="font-medium">Connected</span>
              </div>
            ) : (
              <div>
                <div className="flex items-center gap-3 text-yellow-600 mb-4">
                  <svg className="w-6 h-6" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z" />
                  </svg>
                  <span className="font-medium">Not Connected</span>
                </div>
                <Link href="/connect-telegram" className="btn-primary inline-block">
                  Connect Now
                </Link>
              </div>
            )}
          </div>

          {/* Usage Stats */}
          <div className="card">
            <h2 className="text-lg font-semibold text-gray-900 mb-4">Token Usage</h2>
            {usage && (
              <div>
                <div className="flex justify-between text-sm text-gray-600 mb-2">
                  <span>
                    {usage.status === 'trial' ? 'Trial' : 'Paid'} tokens used
                  </span>
                  <span>
                    {usage.trial_tokens_used.toLocaleString()} / {usage.trial_tokens_limit.toLocaleString()}
                  </span>
                </div>
                <div className="w-full bg-gray-200 rounded-full h-3">
                  <div
                    className={`h-3 rounded-full ${
                      usagePercent > 80 ? 'bg-red-500' : usagePercent > 50 ? 'bg-yellow-500' : 'bg-green-500'
                    }`}
                    style={{ width: `${Math.min(usagePercent, 100)}%` }}
                  />
                </div>
                <div className="mt-4 text-sm text-gray-500">
                  Status: <span className="font-medium capitalize">{usage.status}</span>
                  {usage.total_tokens_purchased > 0 && (
                    <span className="ml-4">
                      Total purchased: {usage.total_tokens_purchased.toLocaleString()}
                    </span>
                  )}
                </div>
              </div>
            )}
          </div>
        </div>

        {/* Token Packages */}
        <div className="card">
          <h2 className="text-lg font-semibold text-gray-900 mb-6">Buy More Tokens</h2>
          <div className="grid sm:grid-cols-2 md:grid-cols-4 gap-4">
            {packages.map((pkg) => (
              <div
                key={pkg.id}
                className="border rounded-lg p-4 text-center hover:border-primary-500 transition-colors"
              >
                <div className="text-2xl font-bold text-gray-900 mb-1">
                  {(pkg.tokens / 1000).toFixed(0)}K
                </div>
                <div className="text-sm text-gray-500 mb-3">tokens</div>
                <div className="text-xl font-bold text-primary-600 mb-4">
                  ${pkg.price_usd}
                </div>
                <button
                  onClick={() => handleBuyTokens(pkg.id)}
                  className="btn-primary w-full text-sm py-2"
                >
                  Buy Now
                </button>
              </div>
            ))}
          </div>
          <p className="text-sm text-gray-500 mt-4 text-center">
            Payments processed securely via Cryptomus (crypto only)
          </p>
        </div>

        {/* Quick Actions */}
        <div className="mt-8">
          <h2 className="text-lg font-semibold text-gray-900 mb-4">Getting Started</h2>
          <div className="grid sm:grid-cols-3 gap-4">
            <div className="card hover:shadow-lg transition-shadow">
              <div className="text-3xl mb-3">🎯</div>
              <h3 className="font-semibold text-gray-900 mb-2">Set Your Goals</h3>
              <p className="text-sm text-gray-600">
                Tell the bot about your goals via Telegram. It will transform them into SMART format.
              </p>
            </div>
            <div className="card hover:shadow-lg transition-shadow">
              <div className="text-3xl mb-3">🔮</div>
              <h3 className="font-semibold text-gray-900 mb-2">Discover Yourself</h3>
              <p className="text-sm text-gray-600">
                Share your birthdate for Bazi and Destiny Matrix calculations to understand your path.
              </p>
            </div>
            <div className="card hover:shadow-lg transition-shadow">
              <div className="text-3xl mb-3">💬</div>
              <h3 className="font-semibold text-gray-900 mb-2">Chat Anytime</h3>
              <p className="text-sm text-gray-600">
                Your AI mentor is available 24/7 via Telegram for advice and support.
              </p>
            </div>
          </div>
        </div>
      </div>
    </main>
  );
}
