-- Glance Test Database Schema
-- This file seeds the test database with sample data for development and testing.

-- Users table
CREATE TABLE users (
    id SERIAL PRIMARY KEY,
    email VARCHAR(255) NOT NULL UNIQUE,
    name VARCHAR(100),
    created_at TIMESTAMP NOT NULL DEFAULT NOW()
);

-- Orders table with foreign key to users
CREATE TABLE orders (
    id SERIAL PRIMARY KEY,
    user_id INTEGER NOT NULL REFERENCES users(id),
    total NUMERIC(10, 2) NOT NULL,
    status VARCHAR(20) NOT NULL DEFAULT 'pending',
    created_at TIMESTAMP NOT NULL DEFAULT NOW()
);

-- Indexes
CREATE INDEX idx_orders_user_id ON orders(user_id);
CREATE INDEX idx_orders_status ON orders(status);
CREATE INDEX idx_users_email ON users(email);

-- Sample users
INSERT INTO users (email, name, created_at) VALUES
    ('alice@example.com', 'Alice Johnson', '2024-01-15 10:30:00'),
    ('bob@example.com', 'Bob Smith', '2024-01-16 14:22:00'),
    ('carol@example.com', NULL, '2024-01-17 09:15:00'),
    ('david@example.com', 'David Brown', '2024-01-18 11:45:00'),
    ('eve@example.com', 'Eve Wilson', '2024-01-19 16:00:00'),
    ('frank@example.com', 'Frank Miller', '2024-01-20 08:30:00'),
    ('grace@example.com', 'Grace Lee', '2024-01-21 13:15:00'),
    ('henry@example.com', 'Henry Taylor', '2024-01-22 10:00:00'),
    ('iris@example.com', NULL, '2024-01-23 15:45:00'),
    ('jack@example.com', 'Jack Anderson', '2024-01-24 09:30:00');

-- Sample orders
INSERT INTO orders (user_id, total, status, created_at) VALUES
    (1, 99.99, 'completed', '2024-01-15 11:00:00'),
    (1, 149.50, 'completed', '2024-01-16 09:30:00'),
    (2, 75.00, 'pending', '2024-01-17 14:00:00'),
    (2, 200.00, 'completed', '2024-01-18 10:15:00'),
    (3, 50.25, 'cancelled', '2024-01-19 16:30:00'),
    (4, 125.75, 'completed', '2024-01-20 11:45:00'),
    (4, 89.99, 'pending', '2024-01-21 08:00:00'),
    (5, 175.00, 'completed', '2024-01-22 13:30:00'),
    (6, 45.50, 'shipped', '2024-01-23 09:15:00'),
    (7, 299.99, 'completed', '2024-01-24 14:45:00'),
    (7, 65.00, 'pending', '2024-01-25 10:30:00'),
    (8, 110.25, 'shipped', '2024-01-26 15:00:00'),
    (9, 85.75, 'completed', '2024-01-27 11:15:00'),
    (10, 195.00, 'pending', '2024-01-28 08:45:00'),
    (1, 55.00, 'shipped', '2024-01-29 12:00:00');
