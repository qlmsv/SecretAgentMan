'use client';

import Link from 'next/link';

export default function Home() {
  return (
    <main className="min-h-screen bg-gradient-to-br from-primary-900 via-primary-800 to-primary-900">
      {/* Hero Section */}
      <div className="container mx-auto px-4 py-16">
        <nav className="flex justify-between items-center mb-16">
          <div className="text-2xl font-bold text-white">AI-Mentor</div>
          <div className="space-x-4">
            <Link href="/login" className="text-white hover:text-primary-200 transition-colors">
              Login
            </Link>
            <Link href="/register" className="btn-primary">
              Get Started
            </Link>
          </div>
        </nav>

        <div className="max-w-4xl mx-auto text-center">
          <h1 className="text-5xl md:text-6xl font-bold text-white mb-6">
            Your Personal AI Assistant for{' '}
            <span className="text-primary-300">Self-Development</span>
          </h1>
          <p className="text-xl text-primary-100 mb-8 max-w-2xl mx-auto">
            Find yourself, earn more, and live happier. AI-powered coaching with Bazi,
            Destiny Matrix analysis, and SMART goal setting.
          </p>
          <div className="flex flex-col sm:flex-row gap-4 justify-center">
            <Link href="/register" className="btn-primary text-lg px-8 py-4">
              Start Free Trial
            </Link>
            <a href="#features" className="btn-secondary text-lg px-8 py-4">
              Learn More
            </a>
          </div>
        </div>
      </div>

      {/* Features Section */}
      <section id="features" className="bg-white py-20">
        <div className="container mx-auto px-4">
          <h2 className="text-4xl font-bold text-center text-gray-900 mb-12">
            Everything You Need for Personal Growth
          </h2>

          <div className="grid md:grid-cols-3 gap-8">
            <FeatureCard
              icon="🎯"
              title="SMART Goals"
              description="Transform your dreams into actionable goals. AI automatically converts your wishes into first-person, present-tense affirmations."
            />
            <FeatureCard
              icon="🔮"
              title="Esoteric Analysis"
              description="Discover your destiny through Bazi (Four Pillars) and Destiny Matrix calculations. Understand your strengths and life path."
            />
            <FeatureCard
              icon="💬"
              title="24/7 AI Coaching"
              description="Get personalized advice anytime via Telegram. Your AI mentor is always ready to help you navigate life's challenges."
            />
            <FeatureCard
              icon="📰"
              title="News Digest"
              description="Stay informed with AI-curated news from tech, AI, and business sources, summarized just for you."
            />
            <FeatureCard
              icon="📱"
              title="Social Media"
              description="Generate engaging content for LinkedIn, Twitter, and Telegram with AI-powered templates and hashtag suggestions."
            />
            <FeatureCard
              icon="💰"
              title="Fair Pricing"
              description="Pay only for what you use. Token-based billing with transparent pricing. Start with 100,000 free trial tokens."
            />
          </div>
        </div>
      </section>

      {/* Pricing Section */}
      <section className="bg-gray-50 py-20">
        <div className="container mx-auto px-4">
          <h2 className="text-4xl font-bold text-center text-gray-900 mb-4">
            Simple, Transparent Pricing
          </h2>
          <p className="text-center text-gray-600 mb-12 max-w-2xl mx-auto">
            Start free, pay only when you need more. No subscriptions, no hidden fees.
          </p>

          <div className="grid md:grid-cols-4 gap-6 max-w-5xl mx-auto">
            <PricingCard tokens="100K" price="$5" popular={false} />
            <PricingCard tokens="500K" price="$20" popular={true} />
            <PricingCard tokens="1M" price="$35" popular={false} />
            <PricingCard tokens="5M" price="$150" popular={false} />
          </div>

          <p className="text-center text-gray-500 mt-8">
            ~50 messages per 100K tokens. Pay with crypto via Cryptomus.
          </p>
        </div>
      </section>

      {/* How It Works */}
      <section className="bg-white py-20">
        <div className="container mx-auto px-4">
          <h2 className="text-4xl font-bold text-center text-gray-900 mb-12">
            How It Works
          </h2>

          <div className="max-w-3xl mx-auto">
            <Step
              number={1}
              title="Create Account"
              description="Sign up with your email in 30 seconds. No credit card required."
            />
            <Step
              number={2}
              title="Connect Telegram"
              description="Link your Telegram account with a simple click. All conversations happen in your favorite messenger."
            />
            <Step
              number={3}
              title="Complete Onboarding"
              description="Tell the AI about yourself. Choose between esoteric or scientific approaches. Set your goals."
            />
            <Step
              number={4}
              title="Start Growing"
              description="Chat with your AI mentor anytime. Get personalized advice, track goals, and transform your life."
            />
          </div>
        </div>
      </section>

      {/* CTA Section */}
      <section className="bg-primary-900 py-20">
        <div className="container mx-auto px-4 text-center">
          <h2 className="text-4xl font-bold text-white mb-6">
            Ready to Transform Your Life?
          </h2>
          <p className="text-xl text-primary-200 mb-8 max-w-2xl mx-auto">
            Join thousands of people who are already using AI-Mentor to achieve their goals.
          </p>
          <Link href="/register" className="btn-primary text-lg px-8 py-4 inline-block">
            Start Your Free Trial
          </Link>
        </div>
      </section>

      {/* Footer */}
      <footer className="bg-gray-900 text-gray-400 py-8">
        <div className="container mx-auto px-4 text-center">
          <p>&copy; 2024 AI-Mentor. Powered by ZeroClaw.</p>
        </div>
      </footer>
    </main>
  );
}

function FeatureCard({
  icon,
  title,
  description,
}: {
  icon: string;
  title: string;
  description: string;
}) {
  return (
    <div className="card hover:shadow-xl transition-shadow">
      <div className="text-4xl mb-4">{icon}</div>
      <h3 className="text-xl font-semibold text-gray-900 mb-2">{title}</h3>
      <p className="text-gray-600">{description}</p>
    </div>
  );
}

function PricingCard({
  tokens,
  price,
  popular,
}: {
  tokens: string;
  price: string;
  popular: boolean;
}) {
  return (
    <div
      className={`card text-center ${
        popular ? 'ring-2 ring-primary-500 transform scale-105' : ''
      }`}
    >
      {popular && (
        <div className="bg-primary-500 text-white text-sm font-semibold px-3 py-1 rounded-full inline-block mb-4">
          Most Popular
        </div>
      )}
      <div className="text-3xl font-bold text-gray-900 mb-2">{tokens}</div>
      <div className="text-lg text-gray-600 mb-4">tokens</div>
      <div className="text-4xl font-bold text-primary-600 mb-4">{price}</div>
      <Link
        href="/register"
        className={`block w-full py-3 rounded-lg font-semibold transition-colors ${
          popular
            ? 'bg-primary-600 text-white hover:bg-primary-700'
            : 'bg-gray-100 text-gray-700 hover:bg-gray-200'
        }`}
      >
        Get Started
      </Link>
    </div>
  );
}

function Step({
  number,
  title,
  description,
}: {
  number: number;
  title: string;
  description: string;
}) {
  return (
    <div className="flex items-start gap-6 mb-8">
      <div className="flex-shrink-0 w-12 h-12 bg-primary-100 text-primary-600 rounded-full flex items-center justify-center text-xl font-bold">
        {number}
      </div>
      <div>
        <h3 className="text-xl font-semibold text-gray-900 mb-2">{title}</h3>
        <p className="text-gray-600">{description}</p>
      </div>
    </div>
  );
}
