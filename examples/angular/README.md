# MobileMesh en Angular

App de Angular (CLI) que usa MobileMesh desde npm. Probada: crea la cuenta y guarda datos.

```sh
ng new mi-app --standalone
cd mi-app && npm install mobilemesh
```

En `angular.json`, dentro de `projects.<nombre>.architect.serve.options`, añade las cabeceras:

```json
"headers": {
  "Cross-Origin-Opener-Policy": "same-origin",
  "Cross-Origin-Embedder-Policy": "require-corp"
}
```

En `src/app/app.component.ts` usa `signal()` para que la vista se refresque; en Angular moderno, sin
zone.js, una propiedad normal no se actualiza sola:

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
    const mesh = await MobileMesh.start({ appId: "mi-app" });
    const tareas = mesh.collection("tareas");
    await tareas.save("t1", { titulo: "Comprar pan" });
    this.total.set((await tareas.all()).length);
  }
}
```

Arranca con `ng serve`.
