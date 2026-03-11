/**
 * JSX Syntax Highlighting Test
 * A React component library demonstrating JSX highlighting.
 */

import React, { useState, useEffect, useCallback, useRef } from 'react';

// Constants
const API_BASE = 'https://api.example.com';
const MAX_RETRIES = 3;

// Simple functional component
function Badge({ label, count, color = 'blue' }) {
    if (count === 0) return null;

    return (
        <span className={`badge badge-${color}`}>
            {label}: <strong>{count}</strong>
        </span>
    );
}

// Component with fragments and conditional rendering
function StatusIndicator({ status, message }) {
    const icons = {
        success: '✓',
        error: '✗',
        loading: '⟳',
    };

    return (
        <>
            <span className={`status-${status}`}>
                {icons[status] || '?'}
            </span>
            {message && <span className="status-message">{message}</span>}
        </>
    );
}

// Component with spread attributes and event handlers
function IconButton({ icon, label, variant = 'default', ...rest }) {
    return (
        <button
            className={`icon-btn icon-btn-${variant}`}
            aria-label={label}
            title={label}
            {...rest}
        >
            <span className="icon">{icon}</span>
        </button>
    );
}

// List rendering with keys
function TagList({ tags, onRemove }) {
    if (!tags || tags.length === 0) {
        return <p className="empty">No tags yet.</p>;
    }

    return (
        <ul className="tag-list">
            {tags.map((tag, index) => (
                <li key={tag.id || index} className="tag-item">
                    <span>{tag.name}</span>
                    {onRemove && (
                        <IconButton
                            icon="×"
                            label={`Remove ${tag.name}`}
                            variant="danger"
                            onClick={() => onRemove(tag.id)}
                        />
                    )}
                </li>
            ))}
        </ul>
    );
}

// Custom hook
function useFetch(url) {
    const [data, setData] = useState(null);
    const [loading, setLoading] = useState(true);
    const [error, setError] = useState(null);
    const abortRef = useRef(null);

    useEffect(() => {
        abortRef.current = new AbortController();
        setLoading(true);

        fetch(url, { signal: abortRef.current.signal })
            .then(res => {
                if (!res.ok) throw new Error(`HTTP ${res.status}`);
                return res.json();
            })
            .then(json => {
                setData(json);
                setError(null);
            })
            .catch(err => {
                if (err.name !== 'AbortError') {
                    setError(err.message);
                }
            })
            .finally(() => setLoading(false));

        return () => abortRef.current?.abort();
    }, [url]);

    return { data, loading, error };
}

// Component using the custom hook
function UserCard({ userId }) {
    const { data: user, loading, error } = useFetch(
        `${API_BASE}/users/${userId}`
    );

    if (loading) {
        return (
            <div className="card card-loading">
                <div className="skeleton skeleton-avatar" />
                <div className="skeleton skeleton-text" />
            </div>
        );
    }

    if (error) {
        return (
            <div className="card card-error">
                <StatusIndicator status="error" message={error} />
            </div>
        );
    }

    return (
        <div className="card user-card">
            {user.avatar ? (
                <img
                    src={user.avatar}
                    alt={`${user.name}'s avatar`}
                    className="avatar"
                    loading="lazy"
                />
            ) : (
                <div className="avatar avatar-placeholder">
                    {user.name[0].toUpperCase()}
                </div>
            )}
            <h3>{user.name}</h3>
            <p className="email">{user.email}</p>
            <TagList tags={user.tags} />
        </div>
    );
}

// Form with controlled inputs and validation
function SearchForm({ onSearch, placeholder = 'Search...' }) {
    const [query, setQuery] = useState('');
    const inputRef = useRef(null);

    const handleSubmit = useCallback(
        (e) => {
            e.preventDefault();
            const trimmed = query.trim();
            if (trimmed) {
                onSearch(trimmed);
            }
        },
        [query, onSearch]
    );

    useEffect(() => {
        inputRef.current?.focus();
    }, []);

    return (
        <form onSubmit={handleSubmit} className="search-form" role="search">
            <input
                ref={inputRef}
                type="search"
                value={query}
                onChange={(e) => setQuery(e.target.value)}
                placeholder={placeholder}
                aria-label="Search"
                className="search-input"
            />
            <button type="submit" disabled={!query.trim()}>
                Search
            </button>
        </form>
    );
}

// Main app component
export default function App() {
    const [searchResults, setSearchResults] = useState([]);
    const userIds = ['u1', 'u2', 'u3'];

    const handleSearch = useCallback((query) => {
        console.log(`Searching for: ${query}`);
        setSearchResults([{ id: 1, name: query }]);
    }, []);

    return (
        <div className="app">
            <header>
                <h1>JSX Syntax Test</h1>
                <Badge label="Users" count={userIds.length} color="green" />
            </header>
            <main>
                <SearchForm onSearch={handleSearch} />
                <section className="user-grid">
                    {userIds.map(id => (
                        <UserCard key={id} userId={id} />
                    ))}
                </section>
                {searchResults.length > 0 && (
                    <section>
                        <h2>Results</h2>
                        <TagList
                            tags={searchResults}
                            onRemove={(id) => {
                                setSearchResults(prev =>
                                    prev.filter(r => r.id !== id)
                                );
                            }}
                        />
                    </section>
                )}
            </main>
            {/* Footer with self-closing component */}
            <footer>
                <StatusIndicator status="success" message="Connected" />
            </footer>
        </div>
    );
}

export { Badge, IconButton, SearchForm, TagList, UserCard, useFetch };
