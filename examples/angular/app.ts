import { Component, signal } from '@angular/core';
import { MobileMesh } from 'mobilemesh';

@Component({
  selector: 'app-root',
  template: `<main style="font-family: system-ui; padding: 24px">
    <h1>MobileMesh en Angular</h1>
    <p>{{ estado() }}</p>
  </main>`,
})
export class App {
  protected readonly estado = signal('arrancando…');

  constructor() {
    this.arrancar();
  }

  private async arrancar() {
    try {
      const mesh = await MobileMesh.start({ appId: 'prueba-angular' });
      const cuenta = await mesh.account();
      const tareas = mesh.collection('tareas');
      await tareas.save('t1', { titulo: 'Comprar pan', hecha: 'no' });
      const pendientes = await tareas.where('hecha', '==', 'no').get();
      this.estado.set(`ANGULAR OK · cuenta ${cuenta.slice(0, 12)}… · pendientes: ${pendientes.length}`);
    } catch (e: any) {
      this.estado.set(`ANGULAR FALLA: ${e.message}`);
    }
  }
}
