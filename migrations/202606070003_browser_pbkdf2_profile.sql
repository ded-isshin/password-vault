ALTER TABLE accounts
    DROP CONSTRAINT accounts_kdf_profile_check;

UPDATE accounts
SET kdf_profile = jsonb_build_object(
    'id', 'pbkdf2-sha256-browser-v1',
    'algorithm', 'PBKDF2-HMAC-SHA-256',
    'iterations', 600000,
    'hash', 'SHA-256'
)
WHERE kdf_profile = jsonb_build_object(
    'id', 'argon2id-browser-v1',
    'algorithm', 'argon2id',
    'memory_kib', 19456,
    'iterations', 2,
    'parallelism', 1
);

ALTER TABLE accounts
    ADD CONSTRAINT accounts_kdf_profile_check CHECK (
        kdf_profile = jsonb_build_object(
            'id', 'pbkdf2-sha256-browser-v1',
            'algorithm', 'PBKDF2-HMAC-SHA-256',
            'iterations', 600000,
            'hash', 'SHA-256'
        )
    );
