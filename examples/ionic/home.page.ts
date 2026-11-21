import { Component, signal } from '@angular/core';
import { MobileMesh } from 'mobilemesh';

@Component({
  selector: 'app-home',
  template: `<ion-header><ion-toolbar><ion-title>MobileMesh en Ionic</ion-title></ion-toolbar></ion-header>
    <ion-content class="ion-padding"><p>{{ estado() }}</p></ion-content>`,
  standalone: false,
})
export class HomePage {
  readonly estado = signal('arrancando…');

  constructor() {
    this.arrancar();
  }

  private async arrancar() {
    try {
      const mesh = await MobileMesh.start({ appId: 'prueba-ionic', workerUrl: new URL('mobilemesh/nucleo-worker.js', document.baseURI) });
      const cuenta = await mesh.account();
      const tareas = mesh.collection('tareas');
      await tareas.save('t1', { titulo: 'Comprar pan', hecha: 'no' });
      const pendientes = await tareas.where('hecha', '==', 'no').get();
      this.estado.set(`IONIC OK · cuenta ${cuenta.slice(0, 12)}… · pendientes: ${pendientes.length}`);
    } catch (e: any) {
      this.estado.set(`IONIC FALLA: ${e.message}`);
    }
  }
}
