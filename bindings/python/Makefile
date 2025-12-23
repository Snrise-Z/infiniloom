.PHONY: help install dev build test clean lint format

help:
	@echo "Infiniloom Python Bindings"
	@echo ""
	@echo "Available targets:"
	@echo "  install    - Install the package"
	@echo "  dev        - Install in development mode"
	@echo "  build      - Build release wheel"
	@echo "  test       - Run tests"
	@echo "  clean      - Clean build artifacts"
	@echo "  lint       - Run linters"
	@echo "  format     - Format code"

install:
	pip install .

dev:
	pip install maturin
	maturin develop

build:
	pip install maturin
	maturin build --release

test:
	pip install pytest
	pytest tests/ -v

clean:
	rm -rf target/
	rm -rf build/
	rm -rf dist/
	rm -rf *.egg-info
	rm -rf python/infiniloom/__pycache__
	rm -rf tests/__pycache__
	rm -rf .pytest_cache
	find . -type f -name "*.pyc" -delete
	find . -type d -name "__pycache__" -delete

lint:
	pip install ruff
	ruff check python/

format:
	pip install ruff
	ruff format python/
	cargo fmt
