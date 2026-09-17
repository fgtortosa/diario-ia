# Exportación automática del diario a los repositorios

**Fecha:** 2026-09-17
**Estado:** aprobado, pendiente de plan de implementación

## Problema

Cada repositorio mantiene a mano un `DIARIO.md`, y el workspace un
`TAREAS-EFECTUADAS.md`. Ese registro depende de que el agente se acuerde de escribirlo
después de haber llamado a `log_task`, y falla de dos maneras distintas:

- **Se olvida.** Nada lo obliga. Si el agente cierra la tarea sin escribir el fichero, el
  repositorio se queda sin rastro y no hay forma de detectarlo.
- **Se duplica el trabajo.** El mismo contenido se escribe dos veces: una en el diario y
  otra en el fichero, a mano y con el riesgo de que difieran.

Hay además un tercer problema, latente pero seguro: **añadir al final de un fichero único
garantiza conflictos**. Dos ramas que registren tareas tocan las dos la última línea de
`DIARIO.md`, y eso es conflicto en cada merge, siempre. En este workspace es la norma, no
la excepción: `redes-ice-netcore` ha tenido tres ramas vivas a la vez esta misma semana.

## Decisión

El diario pasa a ser la **fuente única**. El agente llama a `log_task` y nada más; el
propio servidor escribe en los repositorios, de forma asíncrona, un **fichero nuevo por
entrada** dentro de una carpeta.

```
Agente ──log_task──▶ diario-ia ──asíncrono──▶ <repo>/diario-ia/<fichero>.md
                     (fuente única)     └───▶ ~/codigo/tareas-ia/<fichero>.md
```

La regla que lo hace seguro: **el escritor solo crea ficheros nuevos y nunca modifica uno
existente.** De ahí se siguen dos propiedades que son el motivo de todo el diseño:

- **Cero conflictos de merge**, por construcción: cada rama crea ficheros con nombres que
  no pueden coincidir.
- **Nada se corrompe** si el escritor falla a mitad: como mucho falta un fichero, y la
  siguiente pasada lo escribe.

## Esquema

Dos columnas. Nada más.

```sql
ALTER TABLE application ADD COLUMN repo_path   TEXT;   -- dónde escribir; NULL = no se exporta al repo
ALTER TABLE entry       ADD COLUMN exported_at TEXT;   -- NULL = pendiente de exportar
```

`exported_at` es a la vez el marcado de "ya está" y la cola de trabajo: lo pendiente es
exactamente `WHERE exported_at IS NULL`. No hace falta una tabla de cola.

## Flujo

`log_task` inserta la entrada y **devuelve inmediatamente**. Antes de devolver despierta al
escritor, que:

1. Toma **todas** las entradas con `exported_at IS NULL`, no solo la recién creada.
2. Para cada una, resuelve sus destinos:
   - `<repo_path>/diario-ia/` de su aplicación, si la aplicación tiene `repo_path`.
   - El directorio central, siempre. Sale de `DIARIO_TAREAS_DIR`, por defecto
     `~/codigo/tareas-ia/`. Es configurable porque esa ruta es de esta máquina: un servidor
     desplegado en otro host no la tiene, y ahí el valor por defecto sería un directorio
     suyo. Si la variable está vacía, no se escribe destino central.
3. Crea las carpetas si no existen y escribe el fichero, **saltándose el destino cuyo
   fichero ya exista**.
4. **Solo si todas las escrituras de esa entrada vuelven sin error**, marca `exported_at`.

El salto del punto 3 no es una optimización, es lo que hace correcto el reintento. Una
entrada tiene dos destinos y pueden fallar por separado: si el central se escribe y el del
repositorio falla, la entrada sigue pendiente y se reintenta, y sin esa comprobación el
fichero central se escribiría por duplicado. Como el nombre del fichero es determinista
—fecha, hora, aplicación e `id`—, «ya existe» es una comprobación fiable y basta con ella;
no hace falta marcar cada destino por separado.

Que recorra todas las pendientes y no solo la última es lo que hace que el sistema se
recupere solo: si el servidor estuvo parado, si un repositorio no existía todavía o si el
disco estaba lleno, la siguiente escritura arrastra lo atrasado sin que nadie intervenga.

## Nombre del fichero

```
20260917-125851-redes-ice-netcore-19.md
└─fecha─┘└─hora─┘└─── aplicación ───┘└id┘
```

- **Fecha y hora primero**, con segundos, para que el orden alfabético sea el cronológico.
- **La aplicación**, redundante dentro del repositorio —ahí solo hay una— pero necesaria en
  el directorio central, y así el esquema es uno solo para los dos destinos y el fichero se
  explica solo si acaba en otro sitio.
- **El `id`** de la entrada, que es lo que garantiza unicidad y el enlace de vuelta a la web
  del diario.

El `id` no es decorativo: el export actual ya generó `20260917-125851-18.md` y
`20260917-125851-20.md`, **mismo segundo**, al registrar cuatro entradas seguidas. Sin `id`
se pisan.

Se consideró añadir el título en slug para que la carpeta se lea con un `ls`. Se descarta:
los títulos llevan acentos y puntuación, el slug es una transformación con pérdida que
alguien intentará revertir, y obliga a decidir un corte de longitud. El título va como `H1`
dentro del fichero, que es donde no se pierde nada.

## Registro de repositorios

```
diario repo set <aplicacion> <ruta>   # asocia la aplicación a un repositorio
diario repo list                      # qué aplicaciones escriben y dónde
diario repo status                    # entradas pendientes de exportar, por aplicación
```

Una aplicación sin `repo_path` no escribe en ningún repositorio, pero **sí escribe en el
directorio central**. Así nada se pierde por no haberla registrado todavía, y cuando se
registre, la siguiente pasada arrastra todo su histórico pendiente.

## Errores

Una ruta que no existe, sin permisos o un disco lleno dejan `exported_at` en `NULL` y **no
rompen `log_task`**. La alternativa —que `log_task` falle— es peor: perderías el registro de
la tarea por un problema de disco que no tiene nada que ver con ella.

Como el fallo es silencioso por diseño, hace falta una forma de verlo: `diario repo status`
lista lo pendiente por aplicación, y la cifra debería ser cero salvo justo después de un
fallo.

## Ramas de git

El escritor no sabe en qué rama está el repositorio, y no necesita saberlo. Como solo crea
ficheros nuevos, la entrada aparece como fichero sin rastrear: el agente lo incluye en su
PR, o se queda ahí hasta la siguiente. Nunca modifica una línea que otra rama pueda haber
cambiado, así que no hay conflicto posible.

## Migración

Los `DIARIO.md` y el `TAREAS-EFECTUADAS.md` que ya existen **no se tocan ni se trocean**. Se
quedan como histórico cerrado y el contenido nuevo va a las carpetas. Trocear lo viejo
sería reescribir registros ya cerrados, y la norma del workspace es explícita en que un
registro no se reescribe: los prompts van literales y las entradas son lo que pasó.

Por el mismo motivo, **la migración marca `exported_at` en todas las entradas que ya
existen**, con la fecha de la propia migración. Son las 21 entradas actuales, y su contenido
ya está escrito a mano en los `DIARIO.md` y en `TAREAS-EFECTUADAS.md`: dejarlas pendientes
haría que la primera pasada volcara 21 ficheros duplicando lo que ya está escrito, en
repositorios que además en su mayoría todavía no tienen `repo_path`.

La consecuencia hay que asumirla explícitamente: **el corte es el día de la migración**. Lo
anterior se lee en los ficheros a mano o en la web del diario; lo posterior, en las
carpetas.

## Fuera de alcance

**`TAREAS-PENDIENTES.md` se queda como está.** Se consideró darle el mismo tratamiento y no
encaja: las efectuadas y el diario son **registros** —append-only, nada se retira—, mientras
que las pendientes son **estado**, porque las cosas se resuelven y dejan de estar
pendientes. Con ficheros append-only nada saldría nunca de la lista y a los dos meses no se
distinguiría lo vivo de lo hecho. Necesitaría marcado de resuelto y movimiento de ficheros,
que ya no es exportar un diario sino un gestor de tareas. Si se quiere, se diseña aparte.

## Criterio de terminación

- Una llamada a `log_task` deja el fichero en el repositorio sin que el agente haga nada más.
- Registrar cuatro entradas seguidas produce cuatro ficheros, sin colisión de nombres.
- Parar el servidor, registrar entradas por REST, arrancarlo y registrar una más escribe
  también las atrasadas.
- Una aplicación sin `repo_path` escribe en el directorio central y en ninguno más.
- Un `repo_path` que apunta a una carpeta inexistente deja la entrada pendiente, no rompe
  `log_task`, y `diario repo status` la muestra.
- Dos ramas que registren tareas y hagan merge no producen ningún conflicto.
