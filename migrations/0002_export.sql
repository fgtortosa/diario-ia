-- Exportacion automatica del diario a los repositorios.
--
-- repo_path:   carpeta del repositorio de esa aplicacion, donde se escribe el
--              subdirectorio diario-ia/. NULL = no se exporta al repositorio,
--              solo al directorio central.
-- exported_at: NULL = pendiente de exportar. Hace de marcado y de cola a la vez:
--              lo pendiente es exactamente WHERE exported_at IS NULL, asi que no
--              hace falta una tabla de cola aparte.
ALTER TABLE application ADD COLUMN repo_path TEXT;
ALTER TABLE entry ADD COLUMN exported_at TEXT;

-- Las entradas que ya existian tienen su contenido escrito a mano en los
-- DIARIO.md de cada repositorio y en TAREAS-EFECTUADAS.md. Marcarlas evita que
-- la primera pasada vuelque duplicados de lo que ya esta escrito. El corte es
-- el dia de la migracion: lo anterior se lee en esos ficheros o en la web.
UPDATE entry SET exported_at = datetime('now') WHERE exported_at IS NULL;

-- Indice parcial: solo indexa lo pendiente, que es lo unico que se consulta y
-- que en regimen normal son cero o unas pocas filas.
CREATE INDEX IF NOT EXISTS idx_entry_pendiente ON entry(exported_at) WHERE exported_at IS NULL;
