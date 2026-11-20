# Vozbit 0.5.12 — atajos y portapapeles

## Cambios implementados

- Los cuatro atajos globales se pueden cambiar, guardar y restaurar desde Configuración:
  - transcripción;
  - cancelar dictado;
  - reprocesar último audio;
  - agregar la palabra seleccionada.
- Vozbit registra el nuevo atajo antes de retirar el anterior, conserva el anterior si Windows rechaza el cambio y evita combinaciones duplicadas.
- **Texto y salida** incluye el interruptor **Transcripción a portapapeles**, desactivado de manera predeterminada.
- Cuando está activo, cada transcripción terminada se copia al portapapeles tanto si la entrega al campo funciona como si falla. Una grabación cancelada o una transcripción fallida no modifica el portapapeles.
- La ayuda muestra las combinaciones actualmente guardadas.
- Cancelar funciona durante preparación, grabación o procesamiento: la solicitud se conserva aunque coincida con el arranque del motor, cierra el indicador y descarta el resultado antes de guardarlo, copiarlo o entregarlo.

## Verificación

- Frontend TypeScript y Vite compilados.
- 36 comprobaciones estáticas aprobadas.
- 18 pruebas de correcciones y 6 pruebas del observador de aprendizaje aprobadas.
- 36 pruebas unitarias Rust y 14 pruebas del arnés de formato aprobadas.
- Prueba nativa aislada aprobada: el atajo de cancelar se capturó y guardó desde la interfaz, una pulsación global real detuvo un procesamiento activo, se rechazaron duplicados y se verificaron la persistencia del interruptor y la escritura real en el portapapeles de Windows.
