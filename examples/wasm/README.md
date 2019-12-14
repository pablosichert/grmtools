# How to run this example
1. Install [Node.js / NPM](https://nodejs.org)
2. `cd examples/wasm`
3. `npm install` # Install JS dependencies
4. `npm run build:release` # Build Rust project as WebAssembly binary, create directory with static JavaScript/HTML glue code
5. `npm run up` # Spins up docker container with nginx on http://localhost:8080
6. `open http://localhost:8080` # open in browser
7. Write something in text box
8. Hit "parse" button
9. Observe output in dev tools console
10. (`npm run down` # Shut down docker container)
