# MobileMesh en Ionic (Angular)

App de Ionic con Angular que usa MobileMesh desde npm. Probada: crea la cuenta y guarda datos.

```sh
ionic start mi-app blank --type=angular
cd mi-app && npm install mobilemesh
```

En `angular.json`, dentro de `projects.<nombre>.architect.serve.options`, añade:

```json
"headers": {
  "Cross-Origin-Opener-Policy": "same-origin",
  "Cross-Origin-Embedder-Policy": "require-corp"
},
"prebundle": { "exclude": ["mobilemesh"] }
```

El `prebundle` es obligatorio: si no, la copia preempaquetada rompe la carga del núcleo.

Copia `nucleo-worker.js`, `red.js` y la carpeta `pkg/` de `node_modules/mobilemesh/` a
`src/assets/mobilemesh/`, y declara esa carpeta como asset en `angular.json`:

```json
{ "glob": "**/*", "input": "src/assets/mobilemesh", "output": "mobilemesh" }
```

En `src/app/app.component.ts` arranca con `workerUrl` (Angular e Ionic reescriben `import.meta.url`
y la ruta relativa al trabajador deja de valer):

```ts
import { Component, OnInit, signal } from "@angular/core";
import { MobileMesh } from "mobilemesh";

@Component({
  selector: "app-root",
  standalone: true,
  template: `{{ total() }} tareas`,
})
export class AppComponent implements OnInit {
  total = signal(0);

  async ngOnInit() {
    const mesh = await MobileMesh.start({
      appId: "mi-app",
      workerUrl: new URL("mobilemesh/nucleo-worker.js", document.baseURI),
    });
    const tareas = mesh.collection("tareas");
    await tareas.save("t1", { titulo: "Comprar pan" });
    this.total.set((await tareas.all()).length);
  }
}
```

Arranca con `ionic serve`.
