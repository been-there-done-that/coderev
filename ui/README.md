# Coderev UI

The web interface for interacting with the Universal Code Intelligence Substrate (Coderev).

## Connection to Backend

This UI is served by the Coderev backend via the `serve` command.

```bash
# From the backend directory
cargo run -- serve
```
This starts both the API and the UI (if built and placed in the static dir).

## Development

If you are developing the UI separately:

### Prerequisites
- Node.js & npm/pnpm

### Install Dependencies
```sh
npm install
```

### Dev Server
To run the SvelteKit development server:
```sh
npm run dev
```

### Build for Production
To build the UI assets for embedding in the Rust backend:
```sh
npm run build
```
This produces the static assets needed for the `serve` command.

