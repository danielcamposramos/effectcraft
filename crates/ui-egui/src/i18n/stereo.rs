//! The stereo adapter’s ten-language catalog; Korean covers these new controls only.
use super::ui::current;
pub(super) const LANGUAGES: [&str; 10] = ["en", "pl", "pt-br", "fr", "de", "es", "ja", "ko", "zh-hans", "zh-hant"];
pub(super) const ROWS: &[(&str, [&str; 10])] = &[
    ("Enable 3D", ["Enable 3D", "Włącz 3D", "Ativar 3D", "Activer la 3D", "3D aktivieren", "Activar 3D", "3Dを有効にする", "3D 활성화", "启用 3D", "啟用 3D"]),
    (
        "Render resolution per eye",
        [
            "Render resolution per eye",
            "Rozdzielczość renderowania na oko",
            "Resolução de renderização por olho",
            "Résolution de rendu par œil",
            "Renderauflösung pro Auge",
            "Resolución de renderizado por ojo",
            "片目ごとの描画解像度",
            "눈별 렌더링 해상도",
            "每眼渲染分辨率",
            "每眼算繪解析度",
        ],
    ),
    (
        "Display Settings",
        [
            "Display Settings",
            "Ustawienia ekranu",
            "Configurações de tela",
            "Paramètres d’affichage",
            "Anzeigeeinstellungen",
            "Configuración de pantalla",
            "ディスプレイ設定",
            "디스플레이 설정",
            "显示设置",
            "顯示設定",
        ],
    ),
    (
        "Stereo View",
        [
            "Stereo View",
            "Widok stereoskopowy",
            "Vista estereoscópica",
            "Vue stéréoscopique",
            "Stereoansicht",
            "Vista estereoscópica",
            "ステレオ表示",
            "스테레오 보기",
            "立体视图",
            "立體檢視",
        ],
    ),
    ("Width", ["Width", "Szerokość", "Largura", "Largeur", "Breite", "Ancho", "幅", "너비", "宽度", "寬度"]),
    ("Height", ["Height", "Wysokość", "Altura", "Hauteur", "Höhe", "Altura", "ハイト", "높이", "高度", "高度"]),
    (
        "Could not open Display Settings: {error}",
        [
            "Could not open Display Settings: {error}",
            "Nie można otworzyć ustawień ekranu: {error}",
            "Não foi possível abrir as configurações de tela: {error}",
            "Impossible d’ouvrir les paramètres d’affichage : {error}",
            "Anzeigeeinstellungen konnten nicht geöffnet werden: {error}",
            "No se pudo abrir la configuración de pantalla: {error}",
            "ディスプレイ設定を開けませんでした: {error}",
            "디스플레이 설정을 열 수 없습니다: {error}",
            "无法打开显示设置：{error}",
            "無法開啟顯示設定：{error}",
        ],
    ),
    (
        "The stereo views have different native dimensions.",
        [
            "The stereo views have different native dimensions.",
            "Widoki stereoskopowe mają różne wymiary natywne.",
            "As vistas estéreo têm dimensões nativas diferentes.",
            "Les vues stéréo ont des dimensions natives différentes.",
            "Die Stereoansichten haben unterschiedliche native Abmessungen.",
            "Las vistas estéreo tienen dimensiones nativas diferentes.",
            "左右のステレオ画像のネイティブ寸法が異なります。",
            "스테레오 보기의 기본 크기가 서로 다릅니다.",
            "立体视图的原生尺寸不同。",
            "立體檢視的原生尺寸不同。",
        ],
    ),
    (
        "The stereo view has invalid native dimensions.",
        [
            "The stereo view has invalid native dimensions.",
            "Widok stereoskopowy ma nieprawidłowe wymiary natywne.",
            "A vista estéreo tem dimensões nativas inválidas.",
            "La vue stéréo a des dimensions natives non valides.",
            "Die Stereoansicht hat ungültige native Abmessungen.",
            "La vista estéreo tiene dimensiones nativas no válidas.",
            "ステレオ表示のネイティブ寸法が無効です。",
            "스테레오 보기의 기본 크기가 잘못되었습니다.",
            "立体视图的原生尺寸无效。",
            "立體檢視的原生尺寸無效。",
        ],
    ),
    (
        "The document stage size preserves its original sampling; the desktop scales once.",
        [
            "The document stage size preserves its original sampling; the desktop scales once.",
            "Rozmiar sceny dokumentu zachowuje jej oryginalne próbkowanie; pulpit skaluje obraz tylko raz.",
            "O tamanho do palco do documento preserva sua amostragem original; a área de trabalho redimensiona a imagem uma só vez.",
            "La taille de la scène du document conserve son échantillonnage d’origine ; le bureau redimensionne la vue une seule fois.",
            "Die Bühnengröße des Dokuments erhält die ursprüngliche Abtastung; der Desktop skaliert die Ansicht nur einmal.",
            "El tamaño del escenario del documento conserva su muestreo original; el escritorio escala la vista una sola vez.",
            "ドキュメントのステージサイズは元のサンプリングを維持し、デスクトップが表示を一度だけ拡大縮小します。",
            "문서의 스테이지 크기는 원래 샘플링을 유지하며 데스크톱은 보기를 한 번만 확대하거나 축소합니다.",
            "文档舞台尺寸保留原始采样，桌面只对视图进行一次缩放。",
            "文件舞台尺寸保留原始取樣，桌面只對檢視進行一次縮放。",
        ],
    ),
    (
        "Use document stage size",
        [
            "Use document stage size",
            "Użyj rozmiaru sceny dokumentu",
            "Usar o tamanho do palco do documento",
            "Utiliser la taille de la scène du document",
            "Bühnengröße des Dokuments verwenden",
            "Usar el tamaño del escenario del documento",
            "ドキュメントのステージサイズを使用",
            "문서의 스테이지 크기 사용",
            "使用文档舞台尺寸",
            "使用文件舞台尺寸",
        ],
    ),
    (
        "Open a composition first.",
        [
            "Open a composition first.",
            "Najpierw otwórz kompozycję.",
            "Abra uma composição primeiro.",
            "Ouvrez d’abord une composition.",
            "Öffnen Sie zuerst eine Komposition.",
            "Abre primero una composición.",
            "まずコンポジションを開いてください。",
            "먼저 컴포지션을 여세요.",
            "请先打开合成。",
            "請先開啟合成。",
        ],
    ),
    (
        "Could not show the stereo view",
        [
            "Could not show the stereo view",
            "Nie można wyświetlić widoku stereoskopowego",
            "Não foi possível mostrar a visualização estéreo",
            "Impossible d’afficher la vue stéréoscopique",
            "Die Stereoansicht konnte nicht angezeigt werden",
            "No se pudo mostrar la vista estéreo",
            "ステレオ表示を開けませんでした",
            "스테레오 보기를 표시할 수 없습니다",
            "无法显示立体视图",
            "無法顯示立體視圖",
        ],
    ),
    (
        "Could not open Display Settings",
        [
            "Could not open Display Settings",
            "Nie można otworzyć ustawień wyświetlania",
            "Não foi possível abrir as configurações de tela",
            "Impossible d’ouvrir les paramètres d’affichage",
            "Die Anzeigeeinstellungen konnten nicht geöffnet werden",
            "No se pudo abrir la configuración de pantalla",
            "ディスプレイ設定を開けませんでした",
            "디스플레이 설정을 열 수 없습니다",
            "无法打开显示设置",
            "無法開啟顯示設定",
        ],
    ),
    (
        "Waiting for stereo depth feedback from the desktop.",
        [
            "Waiting for stereo depth feedback from the desktop.",
            "Oczekiwanie na dane głębi stereoskopowej z pulpitu.",
            "Aguardando os dados de profundidade estéreo do ambiente de trabalho.",
            "En attente des données de profondeur stéréoscopique du bureau.",
            "Warten auf Stereo-Tiefendaten vom Desktop.",
            "Esperando los datos de profundidad estéreo del escritorio.",
            "デスクトップからのステレオ深度情報を待っています。",
            "데스크톱의 스테레오 깊이 정보를 기다리는 중입니다.",
            "正在等待桌面的立体深度信息。",
            "正在等待桌面的立體深度資訊。",
        ],
    ),
    (
        "The desktop did not provide stereo depth feedback. Open Display Settings and enable stereo support.",
        [
            "The desktop did not provide stereo depth feedback. Open Display Settings and enable stereo support.",
            "Pulpit nie udostępnił danych głębi stereoskopowej. Otwórz ustawienia wyświetlania i włącz obsługę stereo.",
            "O ambiente de trabalho não forneceu dados de profundidade estéreo. Abra as configurações de tela e ative o suporte estéreo.",
            "Le bureau n’a pas fourni les données de profondeur stéréoscopique. Ouvrez les paramètres d’affichage et activez la prise en charge stéréo.",
            "Der Desktop hat keine Stereo-Tiefendaten bereitgestellt. Öffnen Sie die Anzeigeeinstellungen und aktivieren Sie die Stereo-Unterstützung.",
            "El escritorio no proporcionó datos de profundidad estéreo. Abre la configuración de pantalla y activa la compatibilidad estéreo.",
            "デスクトップからステレオ深度情報が提供されませんでした。ディスプレイ設定でステレオ対応を有効にしてください。",
            "데스크톱이 스테레오 깊이 정보를 제공하지 않았습니다. 디스플레이 설정에서 스테레오 지원을 활성화하세요.",
            "桌面未提供立体深度信息。请打开显示设置并启用立体支持。",
            "桌面未提供立體深度資訊。請開啟顯示設定並啟用立體支援。",
        ],
    ),
];
pub(crate) fn lookup(language: &str, text: &str) -> Option<&'static str> {
    let column = LANGUAGES.iter().position(|code| *code == language)?;
    ROWS.iter().find(|row| row.0 == text).map(|row| row.1[column])
}
pub(crate) fn tr(text: &str) -> &str {
    lookup(current(), text).unwrap_or(text)
}
pub(crate) fn characters(language: &str) -> Vec<char> {
    ROWS.iter().filter_map(|row| lookup(language, row.0)).flat_map(str::chars).collect()
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn every_new_phrase_has_all_ten_languages() {
        assert_eq!(LANGUAGES.len(), 10);
        for (source, values) in ROWS {
            for (language, value) in LANGUAGES.into_iter().zip(values) {
                assert!(!value.is_empty(), "{source} {language}");
                assert_eq!(lookup(language, source), Some(*value));
                assert_eq!(source.matches("{error}").count(), value.matches("{error}").count());
            }
        }
    }
    #[test]
    fn korean_scope_does_not_claim_a_menu_catalog() {
        assert_eq!(lookup("ko", "File"), None);
        assert_ne!(lookup("ko", "Enable 3D"), Some("Enable 3D"));
    }
}
