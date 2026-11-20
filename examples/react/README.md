# MobileMesh en React

App de React (Vite) que usa MobileMesh desde npm. Probada: crea la cuenta, guarda dos tareas y
consulta las pendientes con un filtro.

```sh
npm create vite@latest mi-app -- --template react
cd mi-app && npm install && npm install mobilemesh
```

Copia `vite.config.js` y `src/App.jsx` de esta carpeta, y arranca con `npm run dev`.

Lo único especial es el `vite.config.js`: MobileMesh usa memoria compartida, así que la página
necesita las cabeceras `Cross-Origin-Opener-Policy` y `Cross-Origin-Embedder-Policy`, y hay que
excluir el paquete del preempaquetado de Vite.
