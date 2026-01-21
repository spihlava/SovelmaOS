---
description: Ensure code quality with linting and testing
---

1. Check for formatting:
   ```bash
   cargo fmt --all -- --check
   ```

2. Run Clippy (linter):
   ```bash
   cargo clippy --all-targets -- -D warnings
   ```

3. Run Tests:
   ```bash
   cargo test --all
   ```
