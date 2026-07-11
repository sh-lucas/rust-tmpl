.PHONY: run test lint database

run:
	bacon run

test:
	cargo test

lint:
	cargo clippy --all-targets -- -D warnings
	cargo fmt --check
	@if [ -f database/database.db ]; then \
		sqlite3 database/database.db "SELECT sql || ';' FROM sqlite_master WHERE sql IS NOT NULL AND name NOT LIKE '_sqlx_migrations' AND name NOT LIKE 'sqlite_sequence';" > migrations/schema.sql; \
		echo "migrations/schema.sql updated from database metadata."; \
	else \
		echo "Warning: database/database.db not found, skipping schema.sql generation."; \
	fi

# creates the database, altough sqlite pretty much does it automatically
database:
	rm ./database/database.db || true
	rm ./database/database.db-wal || true
	sqlx database create
	sqlx migrate run
