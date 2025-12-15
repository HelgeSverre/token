/**
 * TSX (TypeScript + JSX) Syntax Highlighting Test
 * This file demonstrates React components with TypeScript.
 */

import React, {
    useState,
    useEffect,
    useCallback,
    useMemo,
    useRef,
    createContext,
    useContext,
    ReactNode,
    FC,
    ComponentProps,
} from 'react';

// Type definitions
interface User {
    id: string;
    name: string;
    email: string;
    avatar?: string;
}

interface ButtonProps {
    variant?: 'primary' | 'secondary' | 'danger';
    size?: 'small' | 'medium' | 'large';
    disabled?: boolean;
    loading?: boolean;
    onClick?: () => void;
    children: ReactNode;
}

type InputProps = Omit<ComponentProps<'input'>, 'size'> & {
    label?: string;
    error?: string;
    size?: 'small' | 'medium' | 'large';
};

// Context
interface ThemeContextType {
    theme: 'light' | 'dark';
    toggleTheme: () => void;
}

const ThemeContext = createContext<ThemeContextType | null>(null);

// Custom hook
function useTheme() {
    const context = useContext(ThemeContext);
    if (!context) {
        throw new Error('useTheme must be used within ThemeProvider');
    }
    return context;
}

// Provider component
const ThemeProvider: FC<{ children: ReactNode }> = ({ children }) => {
    const [theme, setTheme] = useState<'light' | 'dark'>('light');

    const toggleTheme = useCallback(() => {
        setTheme(prev => (prev === 'light' ? 'dark' : 'light'));
    }, []);

    const value = useMemo(() => ({ theme, toggleTheme }), [theme, toggleTheme]);

    return (
        <ThemeContext.Provider value={value}>
            {children}
        </ThemeContext.Provider>
    );
};

// Functional component with props
const Button: FC<ButtonProps> = ({
    variant = 'primary',
    size = 'medium',
    disabled = false,
    loading = false,
    onClick,
    children,
}) => {
    const className = `btn btn-${variant} btn-${size}`;

    return (
        <button
            className={className}
            disabled={disabled || loading}
            onClick={onClick}
            type="button"
        >
            {loading ? <span className="spinner" /> : children}
        </button>
    );
};

// Input component with ref forwarding
const Input = React.forwardRef<HTMLInputElement, InputProps>(
    ({ label, error, size = 'medium', className = '', ...props }, ref) => {
        const id = props.id || `input-${Math.random().toString(36).slice(2)}`;

        return (
            <div className={`input-wrapper input-${size}`}>
                {label && <label htmlFor={id}>{label}</label>}
                <input
                    ref={ref}
                    id={id}
                    className={`input ${error ? 'input-error' : ''} ${className}`}
                    aria-invalid={!!error}
                    aria-describedby={error ? `${id}-error` : undefined}
                    {...props}
                />
                {error && (
                    <span id={`${id}-error`} className="error-message">
                        {error}
                    </span>
                )}
            </div>
        );
    }
);

Input.displayName = 'Input';

// Component with useState and useEffect
const UserProfile: FC<{ userId: string }> = ({ userId }) => {
    const [user, setUser] = useState<User | null>(null);
    const [loading, setLoading] = useState(true);
    const [error, setError] = useState<string | null>(null);
    const abortControllerRef = useRef<AbortController | null>(null);

    useEffect(() => {
        const fetchUser = async () => {
            abortControllerRef.current = new AbortController();
            
            try {
                setLoading(true);
                const response = await fetch(`/api/users/${userId}`, {
                    signal: abortControllerRef.current.signal,
                });
                
                if (!response.ok) {
                    throw new Error('Failed to fetch user');
                }
                
                const data: User = await response.json();
                setUser(data);
                setError(null);
            } catch (err) {
                if (err instanceof Error && err.name !== 'AbortError') {
                    setError(err.message);
                }
            } finally {
                setLoading(false);
            }
        };

        fetchUser();

        return () => {
            abortControllerRef.current?.abort();
        };
    }, [userId]);

    if (loading) {
        return <div className="loading">Loading...</div>;
    }

    if (error) {
        return <div className="error">{error}</div>;
    }

    if (!user) {
        return <div className="not-found">User not found</div>;
    }

    return (
        <div className="user-profile">
            {user.avatar && (
                <img
                    src={user.avatar}
                    alt={`${user.name}'s avatar`}
                    className="avatar"
                />
            )}
            <h2>{user.name}</h2>
            <p>{user.email}</p>
        </div>
    );
};

// Generic component
interface ListProps<T> {
    items: T[];
    renderItem: (item: T, index: number) => ReactNode;
    keyExtractor: (item: T, index: number) => string;
    emptyMessage?: string;
}

function List<T>({
    items,
    renderItem,
    keyExtractor,
    emptyMessage = 'No items',
}: ListProps<T>) {
    if (items.length === 0) {
        return <div className="empty">{emptyMessage}</div>;
    }

    return (
        <ul className="list">
            {items.map((item, index) => (
                <li key={keyExtractor(item, index)}>
                    {renderItem(item, index)}
                </li>
            ))}
        </ul>
    );
}

// Form component
interface FormData {
    name: string;
    email: string;
    message: string;
}

const ContactForm: FC = () => {
    const [formData, setFormData] = useState<FormData>({
        name: '',
        email: '',
        message: '',
    });
    const [errors, setErrors] = useState<Partial<FormData>>({});
    const nameInputRef = useRef<HTMLInputElement>(null);

    useEffect(() => {
        nameInputRef.current?.focus();
    }, []);

    const handleChange = (
        e: React.ChangeEvent<HTMLInputElement | HTMLTextAreaElement>
    ) => {
        const { name, value } = e.target;
        setFormData(prev => ({ ...prev, [name]: value }));
        setErrors(prev => ({ ...prev, [name]: undefined }));
    };

    const handleSubmit = async (e: React.FormEvent) => {
        e.preventDefault();
        
        const newErrors: Partial<FormData> = {};
        if (!formData.name) newErrors.name = 'Name is required';
        if (!formData.email) newErrors.email = 'Email is required';
        if (!formData.message) newErrors.message = 'Message is required';

        if (Object.keys(newErrors).length > 0) {
            setErrors(newErrors);
            return;
        }

        // Submit form
        console.log('Submitting:', formData);
    };

    return (
        <form onSubmit={handleSubmit} className="contact-form">
            <Input
                ref={nameInputRef}
                name="name"
                label="Name"
                value={formData.name}
                onChange={handleChange}
                error={errors.name}
            />
            <Input
                name="email"
                type="email"
                label="Email"
                value={formData.email}
                onChange={handleChange}
                error={errors.email}
            />
            <div className="input-wrapper">
                <label htmlFor="message">Message</label>
                <textarea
                    id="message"
                    name="message"
                    value={formData.message}
                    onChange={handleChange}
                    rows={5}
                />
                {errors.message && (
                    <span className="error-message">{errors.message}</span>
                )}
            </div>
            <Button type="submit" variant="primary">
                Send Message
            </Button>
        </form>
    );
};

// App component
const App: FC = () => {
    const users: User[] = [
        { id: '1', name: 'Alice', email: 'alice@example.com' },
        { id: '2', name: 'Bob', email: 'bob@example.com' },
    ];

    return (
        <ThemeProvider>
            <div className="app">
                <header>
                    <h1>TSX Syntax Test</h1>
                </header>
                <main>
                    <section>
                        <h2>User List</h2>
                        <List
                            items={users}
                            keyExtractor={user => user.id}
                            renderItem={user => (
                                <UserProfile userId={user.id} />
                            )}
                        />
                    </section>
                    <section>
                        <h2>Contact Form</h2>
                        <ContactForm />
                    </section>
                </main>
            </div>
        </ThemeProvider>
    );
};

export default App;
export { Button, Input, List, ThemeProvider, useTheme };
