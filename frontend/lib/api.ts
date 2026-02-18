const API_BASE = process.env.NEXT_PUBLIC_API_URL || '';

export interface AuthResponse {
  user_id: string;
  token: string;
}

export interface TelegramLinkResponse {
  link: string;
}

export interface TelegramStatusResponse {
  connected: boolean;
  telegram_username?: string;
}

export interface UsageResponse {
  trial_tokens_used: number;
  trial_tokens_limit: number;
  status: 'trial' | 'active' | 'expired';
  total_tokens_purchased: number;
}

export interface TokenPackage {
  id: string;
  tokens: number;
  price_usd: number;
}

class ApiClient {
  private token: string | null = null;

  setToken(token: string) {
    this.token = token;
    if (typeof window !== 'undefined') {
      localStorage.setItem('auth_token', token);
    }
  }

  getToken(): string | null {
    if (this.token) return this.token;
    if (typeof window !== 'undefined') {
      this.token = localStorage.getItem('auth_token');
    }
    return this.token;
  }

  clearToken() {
    this.token = null;
    if (typeof window !== 'undefined') {
      localStorage.removeItem('auth_token');
    }
  }

  private async request<T>(
    endpoint: string,
    options: RequestInit = {}
  ): Promise<T> {
    const headers: HeadersInit = {
      'Content-Type': 'application/json',
      ...options.headers,
    };

    const token = this.getToken();
    if (token) {
      (headers as Record<string, string>)['Authorization'] = `Bearer ${token}`;
    }

    const response = await fetch(`${API_BASE}${endpoint}`, {
      ...options,
      headers,
    });

    if (!response.ok) {
      const error = await response.json().catch(() => ({}));
      throw new Error(error.error || `HTTP error ${response.status}`);
    }

    return response.json();
  }

  async register(email: string, password: string): Promise<AuthResponse> {
    const response = await this.request<AuthResponse>('/api/auth/register', {
      method: 'POST',
      body: JSON.stringify({ email, password }),
    });
    this.setToken(response.token);
    return response;
  }

  async login(email: string, password: string): Promise<AuthResponse> {
    const response = await this.request<AuthResponse>('/api/auth/login', {
      method: 'POST',
      body: JSON.stringify({ email, password }),
    });
    this.setToken(response.token);
    return response;
  }

  async getTelegramLink(): Promise<TelegramLinkResponse> {
    return this.request<TelegramLinkResponse>('/api/auth/telegram-link');
  }

  async getTelegramStatus(): Promise<TelegramStatusResponse> {
    return this.request<TelegramStatusResponse>('/api/auth/telegram-status');
  }

  async getUsage(): Promise<UsageResponse> {
    return this.request<UsageResponse>('/api/usage');
  }

  async getPackages(): Promise<TokenPackage[]> {
    return this.request<TokenPackage[]>('/api/payment/packages');
  }

  async createPayment(packageId: string): Promise<{ payment_url: string; order_id: string }> {
    return this.request('/api/payment/create', {
      method: 'POST',
      body: JSON.stringify({ package: packageId }),
    });
  }

  isAuthenticated(): boolean {
    return !!this.getToken();
  }

  logout() {
    this.clearToken();
  }
}

export const api = new ApiClient();
