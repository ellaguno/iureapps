# Iurefficient Apps (iureapps)

Lanzador e instalador ligero, en Rust, de las apps de escritorio de
[Iurefficient](https://iurefficient.com): **iureditor, IureTranscribe, IureOCR, IureDav,
IureVoz, IureDesk e IureProject**. Un solo ejecutable (~7 MB) para Linux, Windows y macOS.

La ventana muestra las apps en **mosaico** (o en tabla), como el cajón de apps de GNOME:
icono, nombre y un distintivo de estado en la esquina del icono (✓ instalada, ↑ actualización,
↓ disponible, ⊘ no disponible, con el icono atenuado), con filtros (todas, instaladas,
actualizaciones, no instaladas) y búsqueda. Un clic abre la **ficha** de la app: portada con su
color, la acción principal (Abrir en verde, Instalar en azul, Actualizar en ámbar),
Desinstalar, aceleración y formato, capturas de pantalla (un clic las amplía), versión
instalada y última, tamaño del instalador y novedades. Esc o «‹ Todas las apps» vuelve al
mosaico; el clic derecho en un icono ofrece la acción principal y Desinstalar sin abrir la
ficha.

Qué hace:

- **Sabe qué está instalado** y en qué versión: AppImage/.app que instaló él mismo, y
  también paquetes `.deb`/`.rpm` (vía `dpkg`/`rpm`), programas de Windows (registro
  `Uninstall`) y apps de macOS (`/Applications`, `~/Applications`).
- **Consulta la última versión** de cada app en GitHub Releases (y, si la API limita las
  consultas anónimas, cae al `latest.json` del actualizador de Tauri).
- **Instala, actualiza, abre y desinstala**, una a una o «Actualizar todo».
- **Conoce el equipo**: sistema, distribución y familia de paquetes, CPU (y AVX2), memoria,
  GPU, driver de NVIDIA (`nvidia-smi`), Vulkan y FUSE.
- **Escoge la variante adecuada de IureTranscribe**: CUDA si hay NVIDIA con driver ≥ 525
  (en Linux sólo en `.deb`), Vulkan para AMD/Intel o NVIDIA sin CUDA, Metal en Apple
  Silicon y CPU en otro caso. Se puede cambiar a mano; muestra por qué recomienda cada una.
- **Cuenta de Iurefficient para todas las apps** (mosaico «00»): se inicia sesión una vez
  (con código de verificación si la cuenta lo pide) y el lanzador hace lo mismo que cada
  app con [`iurefficient-connect`](https://github.com/ellaguno/iurefficient-connect): guarda
  la sesión en el llavero del sistema, crea la contraseña de aplicación WebDAV de este
  equipo si no hay ninguna y deja la cuenta como «cuenta activa». IureDav, IureTranscribe,
  IureOCR e iureditor la toman al abrirse si no tienen otra configurada. «Abrir
  Iurefficient» abre la instancia en el navegador. La ficha de la cuenta enlaza también la app
  móvil (iPhone y iPad por TestFlight; Android, próximamente).
- **Carpeta «Más apps»**: otras apps de Iurefficient que el lanzador no instala (el propio
  lanzador e [iureTI](https://github.com/ellaguno/iureTI), el sensor de inventario). Cada
  una tiene su ficha, con capturas, versión y sus formas de instalarla: descargas por
  sistema, o comandos de instalación y de Docker que se copian al portapapeles. Se agregan
  en `EXTRAS` de `src/catalog.rs`; las ligas móviles, en `MOBILE_IOS` y `MOBILE_ANDROID`.
- **Se actualiza solo**: cuando hay una versión nueva del lanzador, «Actualizar y reiniciar»
  baja el archivo que corresponde a cómo está instalado (`.deb`/`.rpm` con `pkexec`, el
  ejecutable suelto, el `.exe` de Windows o el `.dmg` de macOS), comprueba su firma, se
  reemplaza y vuelve a abrirse. Si la release no está firmada o no hay versión para el equipo,
  ofrece descargarla a mano. También `iureapps actualizar-lanzador`.
- **Tema claro u oscuro**: sigue al del sistema (en Linux lo lee del portal del escritorio,
  del tema GTK de GNOME o del esquema de KDE, y cambia en vivo si cambias el del sistema; en
  Windows y macOS, el que informa la ventana). En *Ajustes → Apariencia* se puede fijar
  «Clara» u «Oscura».
- **Una sola ventana**: si ya está abierto, volver a abrirlo trae al frente la ventana existente.
- **Verifica la firma minisign** (`.sig`) de cada instalador con la clave pública del
  actualizador de cada app antes de instalarlo; las apps que no firman (IureDesk) se
  comprueban contra el `SHA256SUMS` de su release.

## Cómo instala en cada sistema

| Sistema | Formato | Dónde |
| --- | --- | --- |
| Linux | AppImage (por defecto, sin root) | `~/.local/share/iureapps/apps/` + entrada en el menú |
| Linux | `.tar.gz` con los binarios (IureProject) | `~/.local/share/iureapps/apps/<app>/` + entrada en el menú |
| Linux | `.deb` / `.rpm` (CUDA, IureVoz, o si no hay FUSE) | `pkexec apt-get install` / `dnf install` |
| Windows | instalador `.exe` en modo silencioso: NSIS (`/S`) o Inno Setup en IureVoz (`/VERYSILENT`) | lo que defina cada app (IureDav pide administrador; IureVoz, sólo el usuario) |
| macOS | `.app.tar.gz` (IureVoz, apps Tauri) o `.dmg` (IureDesk) | `/Applications` o `~/Applications`, sin cuarentena |

Sin `libfuse2` los AppImage se ejecutan con `APPIMAGE_EXTRACT_AND_RUN=1`.

## Uso

```bash
iureapps                      # ventana
iureapps equipo               # lo detectado del equipo y la variante recomendada
iureapps lista                # instalada / última / estado de cada app
iureapps instalar iuretranscribe --variante vulkan --formato appimage
iureapps actualizar           # todas las instaladas
iureapps abrir iureditor
iureapps desinstalar iureocr
iureapps conectar 2.ds.iurefficient.com yo@despacho.com   # pide contraseña (y código)
iureapps cuenta               # estado de la cuenta compartida
iureapps iurefficient         # abre la instancia en el navegador
iureapps desconectar
iureapps actualizar-lanzador # instala la última versión de iureapps
```

IureDesk publica sus instaladores (firmados con minisign) en su propio repo,
`ellaguno/iuredesk`; IureProject, en el repo de descargas público
`ellaguno/iureproject-releases`. Para repos privados, configura un token de GitHub de
sólo lectura en *Ajustes* o con la variable `IUREAPPS_GITHUB_TOKEN`. El token también sube
el límite de la API de 60 a 5000 consultas por hora.

## Firma de las releases

Cada archivo de la release lleva su `.sig`, firmado con la misma clave del actualizador de Tauri
que iureditor, IureTranscribe e IureDav (clave pública `8F1B48E168561525`). Los workflows la
toman de los secrets `TAURI_SIGNING_PRIVATE_KEY` y `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`
(los mismos valores que en esos repos). Sin ellos la release se publica igual, sin `.sig`, y el
lanzador no se actualiza solo: ofrece la descarga.

## Capturas de cada app

La ficha muestra las capturas que cada app guarda en una carpeta fija de su repo (el de sus
releases), en la rama por defecto:

```text
docs/screenshots/1.png
docs/screenshots/2.png         … hasta 6, numeradas en orden y sin saltos
docs/screenshots/captions.txt  opcional: un pie de foto por línea, en el mismo orden
```

Basta con subir las imágenes ahí; no hace falta tocar el lanzador. Sólo PNG, de 8 MB y 4096 px
por lado como mucho. Se leen de `raw.githubusercontent.com` (sin gastar consultas de la API)
al abrir la ficha, una vez por sesión o al pulsar *Comprobar*, y se guardan en
`<datos>/shots/<app>/` para verlas sin conexión.

| App | Carpeta |
| --- | --- |
| iureditor | `ellaguno/iureditor` → `docs/screenshots/` |
| IureTranscribe | `ellaguno/iuretranscribe` → `docs/screenshots/` |
| IureOCR | `ellaguno/iureocr` → `docs/screenshots/` |
| IureDav | `ellaguno/iuredav` → `docs/screenshots/` |
| IureVoz | `ellaguno/iurevoz` → `docs/screenshots/` |
| IureDesk | `ellaguno/iuredesk` → `docs/screenshots/` |
| IureProject | `ellaguno/iureproject-releases` → `docs/screenshots/` |

Las novedades de cada versión (el texto de la release en GitHub) se muestran con su formato
Markdown: títulos, listas, negrita, código, citas y enlaces.

## Recursos

Iconos y fuentes incluidos (Inter y JetBrains Mono, subconjunto latino): ver
[`assets/ATTRIBUTION.md`](assets/ATTRIBUTION.md). El icono y el logotipo de Iurefficient
están en SVG en `assets/brand/`; `build.rs` los pasa a PNG al compilar (el logotipo, con las
letras claras para el fondo oscuro).

## Compilar

En Linux hace falta `libdbus-1-dev` (llavero por Secret Service, como en las apps).

```bash
cargo build --release
cargo test
cargo test -- --ignored   # descarga real de iureditor, verifica la firma, instala y desinstala
IUREAPPS_WINDOW_SIZE=1280x900 cargo run   # tamaño inicial de la ventana (capturas)
```

Cuenta compartida contra un doble de la API (usuario `ana@despacho.com`, contraseña
`secreta`, código `123456` con `--totp`):

```bash
python3 tests/api-falsa.py 8099 --totp &
iureapps conectar http://127.0.0.1:8099 ana@despacho.com
```

El conector guarda la instancia sin esquema y la vuelve a abrir por `https`, así que con este
doble (`http`) la sesión aparece como caducada al reabrir el lanzador; con una instancia real
no pasa.

Al subir una etiqueta `vX.Y.Z`, o al lanzar `build.yml` a mano con la entrada `release`
(`vX.Y.Z`), el workflow publica la release con:

| Archivo | Para |
| --- | --- |
| `iureapps_X.Y.Z-1_amd64.deb` | Debian, Ubuntu y derivadas (`sudo apt install ./iureapps_…deb`) |
| `iureapps-X.Y.Z-1.x86_64.rpm` | Fedora, openSUSE y derivadas (`sudo dnf install ./iureapps-…rpm`) |
| `iureapps-linux-x64.tar.gz` | cualquier Linux x64 (binario suelto) |
| `iureapps-windows-x64.exe` | Windows |
| `iureapps-macos-universal.dmg` | macOS (Apple Silicon e Intel), firmado y notarizado |
| `iureapps-macos-arm64.tar.gz`, `iureapps-macos-x64.tar.gz` | macOS, binario suelto sin firmar |

Los paquetes instalan el binario en `/usr/bin`, la entrada del menú y el icono. En macOS,
el `.dmg` trae `Iurefficient Apps.app` (`com.sesolibre.iureapps`), universal, firmada con
Developer ID y notarizada: se arrastra a Aplicaciones y abre sin avisos. Los `.tar.gz` de
macOS no están firmados: la primera vez, clic derecho → Abrir (o
`xattr -d com.apple.quarantine iureapps`).

### Firma de macOS

`.github/workflows/macos.yml` arma el `.app` (plantilla `assets/macos/Info.plist`; versión de
`Cargo.toml` y build = número de ejecución), lo firma con hardened runtime, lo mete en un
`.dmg` firmado, lo notariza con `notarytool` y la llave de App Store Connect (un solo envío,
que cubre la app) y lo engrapa. Si Apple tarda, el workflow «Estado de notarización» consulta
los envíos desde Linux. Los secrets se cargan una sola vez desde la máquina con los
certificados (usa `apple_devid.key` y su `.cer`, o `devid.p12`):

```bash
scripts/apple-secrets.sh ~/apple-certs
```
 El icono (PNG y el `.icns` de macOS) se regenera con `python3 scripts/icono.py`. El lanzador avisa cuando hay una versión suya más nueva.

### Microsoft Store (MSIX)

El job de Windows de `build.yml` genera dos artefactos (no van a la release):

- `msix-store`: `iureapps-windows-x64.msix`, sin firmar, para subir a Partner Center (la
  Store lo firma).
- `msix-prueba`: el mismo paquete firmado con un certificado de prueba y su `.cer`. Para
  instalarlo a mano: importar el `.cer` en *Equipo local → Personas de confianza* y abrir el
  `.msix`.

La identidad del paquete sale de Partner Center (*Identidad del producto*) y va en tres
variables del repo (*Settings → Secrets and variables → Actions → Variables*):
`MSIX_IDENTITY_NAME` (`Package/Identity/Name`), `MSIX_PUBLISHER` (`Package/Identity/Publisher`,
`CN=…`) y `MSIX_PUBLISHER_DISPLAY_NAME` (`Package/Properties/PublisherDisplayName`). Sin ellas
el paquete lleva una identidad de prueba que Partner Center rechaza.

El manifiesto (`assets/windows/msix/AppxManifest.xml`) desactiva la virtualización de AppData
y del registro (capacidad restringida `unvirtualizedResources`) para que lo que instala el
lanzador quede en su sitio real; al enviarlo hay que justificarla. Instalado desde la Store,
el lanzador no se actualiza solo: lo actualiza la Store.

La versión del paquete es la de `Cargo.toml` con el primer número más 1 y un `.0` al final
(`0.1.17` → `1.1.17.0`), porque la Store no acepta un primer número 0 y exige el cuarto en 0;
es el mismo esquema que IureProject. Cada envío necesita una versión mayor que la anterior.

## Agregar una app

Una entrada en `APPS` de `src/catalog.rs`: repo (el de descargas si el código es privado),
nombre de producto, sistemas, si tiene variantes de GPU y la clave pública del actualizador.
Si sigue el esquema de nombres de los instaladores Tauri de la suite
(`<Producto>_<ver>_<n>-<plataforma>[-cuda|-vulkan].<ext>`) o el de cargo-packager
(`Packaging::Packager`, como IureDesk), no hace falta nada más.

## Siguientes fases

- **Fase 2 (resto)**: miniaturas o capturas de cada app (`docs/media` de cada repo) en una
  vista de detalle, e icono propio para IureProject (hoy se dibuja en código; el de IureVoz, que también se
  dibuja aquí, es ya el ícono de la app).
- Cuenta: elegir entre varias cuentas o instancias, y ver o revocar las contraseñas WebDAV y
  los tokens MCP del equipo.
- Autoactualización del lanzador (hoy sólo avisa y enlaza la descarga).
- Comprobar requisitos por app: Tesseract/idiomas de IureOCR, rclone/WinFsp/macFUSE de
  IureDav, GNOME en Wayland de IureVoz.
- Modelos de Whisper: ver los instalados y descargar el adecuado según RAM/VRAM.
- Canal beta (prereleases) por app y «volver a la versión anterior».
- Icono en la bandeja con aviso de actualizaciones.
