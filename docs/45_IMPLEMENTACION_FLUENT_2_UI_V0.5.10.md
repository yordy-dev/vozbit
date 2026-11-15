# Vozbit 0.5.10 — interfaz Fluent Design 2

## Resultado

Se aplicó Fluent Design 2 a la aplicación principal sin modificar la pastilla flotante ni la lógica de dictado.

## Cambios

- Acción principal siempre visible sobre la navegación inferior.
- Recuperación directa para motor local, atajo y errores de configuración.
- Inicio compacto a 360 px sin desplazamiento horizontal.
- Controles de al menos 36 px y navegación de 57 px en la prueba real.
- Configuración con seis categorías, iconos, filas uniformes y mejor jerarquía.
- Historial y estados vacíos simplificados.
- Diccionario con explicación breve y acciones destructivas diferenciadas.
- Tutorial compacto sin cubrir sus objetivos.
- Compatibilidad con colores forzados y movimiento reducido.
- Estilos nuevos limitados a `.vt-app`; ninguna regla nueva afecta `.overlay-shell`.

## Verificación

- `npm test`: aprobado.
- TypeScript y Vite: aprobados.
- 26 comprobaciones estáticas: aprobadas.
- 18 pruebas de diferencias de corrección: aprobadas.
- 6 correcciones observadas de extremo a extremo: aprobadas.
- `cargo fmt --check` y `cargo check`: aprobados.
- 32 pruebas Rust: aprobadas.
- Smoke nativo: seis categorías, modelos disponibles, atajo registrado y cero desbordamiento horizontal.
- Tutorial: cinco pasos, cero superposición a 440 × 500.
- Pastilla: capturas de grabación, procesamiento y error; archivos visuales sin cambios.
- Navegador: cero errores actuales de consola.

## Referencias visuales

- `artifacts/ui-before-compact-360.png`
- `artifacts/ui-after-fluent-home-360.png`
- `artifacts/ui-after-fluent-settings-960.png`
- `artifacts/ui-after-fluent-dictionary-360.png`
- `artifacts/ui-after-fluent-history-360.png`
- `artifacts/ui-after-fluent-tutorial-360.png`
- `artifacts/ui-after-fluent-forced-colors.png`
- `artifacts/fluent-smoke/indicador-grabando-v051.png`
- `artifacts/fluent-smoke/indicador-procesando-v051.png`
- `artifacts/fluent-smoke/indicador-error-v051.png`

