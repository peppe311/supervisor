//! Display native encoded results only. Paths and URLs never grant read access.
use super::*;
use base64::{Engine as _, engine::general_purpose::STANDARD};
use std::io::Cursor;

// Presentation resource bounds, not model/turn/tool limits. Native history keeps
// the original result even when a preview cannot be safely decoded by the UI.
const MAX_ENCODED: usize = 24 * 1024 * 1024;

pub(super) fn output(thread: &Thread, turn: &Turn, item: &Item) -> Option<Value> {
    if item.value["type"] != "imageGeneration"
        || !item.completed
        || !item.value["failure"].is_null()
        || item.value["status"] != "completed"
    {
        return None;
    }
    let mut row = base(thread, turn, &format!("{}:image-output", item.id));
    row["kind"] = json!("native_media");
    row["streaming"] = json!(false);
    row["text"] = json!("");
    let value = &item.value;
    let media = match inline_image(value["result"].as_str().unwrap_or_default()) {
        Ok(preview) => preview,
        Err(reason) => json!({"state":"unavailable","label":reason}),
    };
    row["nativeMedia"] = media;
    Some(row)
}

fn inline_image(result: &str) -> Result<Value, &'static str> {
    let unavailable = "Inline image unavailable. Codex's original result is retained in native history; local paths and remote URLs are not opened automatically.";
    let encoded = result
        .strip_prefix("data:image/png;base64,")
        .or_else(|| result.strip_prefix("data:image/jpeg;base64,"))
        .unwrap_or(result);
    if encoded.is_empty() || encoded.len() > MAX_ENCODED {
        return Err(
            "Inline image unavailable: no embedded result, or the result exceeds the preview size supported by this client. Native history is unchanged.",
        );
    }
    let bytes = STANDARD.decode(encoded).map_err(|_| unavailable)?;
    let format = image::guess_format(&bytes).map_err(|_| unavailable)?;
    let mime = match format {
        image::ImageFormat::Png => "image/png",
        image::ImageFormat::Jpeg => "image/jpeg",
        _ => return Err(unavailable),
    };
    let (width, height) = image::ImageReader::with_format(Cursor::new(&bytes), format)
        .into_dimensions()
        .map_err(|_| unavailable)?;
    if width == 0
        || height == 0
        || width > 16_384
        || height > 16_384
        || u64::from(width) * u64::from(height) > 64_000_000
    {
        return Err(
            "Inline image unavailable: its dimensions exceed the preview supported by this client. Native history is unchanged.",
        );
    }
    Ok(
        json!({"state":"ready","label":"Generated image","previewDataUrl":format!("data:{mime};base64,{encoded}"),"width":width,"height":height}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "Explicit retained native image probe directory; nonvisual decoding and shared main/graph projection only"]
    fn captured_native_image_decodes_and_projects_from_live_and_history() {
        let directory = std::path::PathBuf::from(
            std::env::var_os("CENTRAL_AGENT_TEST_NATIVE_IMAGE_DIR")
                .expect("Set the exact directory retained by image_generation_probe"),
        );
        assert!(directory.is_absolute());
        let directory = directory.canonicalize().unwrap();
        assert!(directory.starts_with(std::env::temp_dir().canonicalize().unwrap()));
        assert!(
            directory
                .file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("central-native-image-")
        );
        let event: Value = serde_json::from_slice(
            &std::fs::read(directory.join("native-image-event.json")).unwrap(),
        )
        .unwrap();
        let history: Value = serde_json::from_slice(
            &std::fs::read(directory.join("native-image-history.json")).unwrap(),
        )
        .unwrap();
        let thread = event["threadId"].as_str().unwrap();
        let turn = event["turnId"].as_str().unwrap();
        assert_eq!(history["thread"]["id"], thread);
        assert_eq!(event["item"]["type"], "imageGeneration");
        assert_eq!(event["item"]["status"], "completed");
        assert!(event["item"]["failure"].is_null());
        assert!(
            history["thread"]["turns"]
                .as_array()
                .unwrap()
                .iter()
                .any(|t| t["id"] == turn
                    && t["items"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .any(|i| i["id"] == event["item"]["id"]
                            && i["result"] == event["item"]["result"]))
        );
        let mut mirror = central_agent_codex_runtime::mirror::Mirror::default();
        mirror.notify("item/completed", &event).unwrap();
        let live = messages(mirror.thread(thread).unwrap());
        let row = live
            .iter()
            .find(|r| r["kind"] == "native_media")
            .expect("Shared main/graph projection omitted native image");
        assert_eq!(row["nativeMedia"]["state"], "ready");
        let url = row["nativeMedia"]["previewDataUrl"].as_str().unwrap();
        let (_, encoded) = url.split_once(";base64,").unwrap();
        let decoded = image::load_from_memory(&STANDARD.decode(encoded).unwrap()).unwrap();
        assert_eq!(row["nativeMedia"]["width"], decoded.width());
        assert_eq!(row["nativeMedia"]["height"], decoded.height());
        mirror.hydrate(&history["thread"], 0).unwrap();
        let loaded = messages(mirror.thread(thread).unwrap());
        let restored = loaded
            .iter()
            .find(|r| r["id"] == row["id"])
            .expect("Native image identity changed on history load");
        assert_eq!(restored["nativeMedia"], row["nativeMedia"]);
        // A replay artifact for checking the TypeScript boundary, not a new
        // native event or a read of the server's savedPath.
        std::fs::write(
            directory.join("native-image-row.json"),
            serde_json::to_vec(row).unwrap(),
        )
        .unwrap();
        println!(
            "PASS: actual native image fully decoded ({}x{}), identical live/history shared main/graph media projection; savedPath not followed, no visual test.",
            decoded.width(),
            decoded.height()
        );
    }

    fn fixture(format: image::ImageFormat) -> String {
        let mut bytes = Cursor::new(Vec::new());
        image::DynamicImage::new_rgb8(8, 6)
            .write_to(&mut bytes, format)
            .unwrap();
        STANDARD.encode(bytes.into_inner())
    }
    #[test]
    fn native_generated_preview_validates_inline_png_and_jpeg_without_fetching() {
        for (format, mime) in [
            (image::ImageFormat::Png, "image/png"),
            (image::ImageFormat::Jpeg, "image/jpeg"),
        ] {
            let encoded = fixture(format);
            for result in [&encoded, &format!("data:{mime};base64,{encoded}")] {
                let view = inline_image(result).unwrap();
                assert_eq!(view["width"], 8);
                assert_eq!(view["height"], 6);
                assert_eq!(
                    view["previewDataUrl"],
                    format!("data:{mime};base64,{encoded}")
                );
            }
        }
        for invalid in [
            "",
            "https://example.invalid/private.png",
            "C:/private/image.png",
            "data:image/svg+xml;base64,PHN2Zz4=",
            "iVBORw0KGgo=",
        ] {
            assert!(inline_image(invalid).is_err());
        }
        assert!(inline_image(&"A".repeat(MAX_ENCODED + 1)).is_err());
    }
    #[test]
    fn native_generated_output_requires_completion_and_keeps_history_identity() {
        let encoded = fixture(image::ImageFormat::Png);
        let mut mirror = central_agent_codex_runtime::mirror::Mirror::default();
        let mut item = json!({"id":"image","type":"imageGeneration","status":"inProgress","result":encoded,"failure":null});
        mirror
            .notify(
                "item/started",
                &json!({"threadId":"t","turnId":"r","item":item}),
            )
            .unwrap();
        assert_eq!(messages(mirror.thread("t").unwrap()).len(), 1);
        item["status"] = json!("completed");
        mirror
            .notify(
                "item/completed",
                &json!({"threadId":"t","turnId":"r","item":item}),
            )
            .unwrap();
        let live = messages(mirror.thread("t").unwrap());
        assert_eq!(live.len(), 2);
        assert_eq!(live[1]["kind"], "native_media");
        assert_eq!(live[1]["nativeMedia"]["state"], "ready");
        assert!(!live[0].to_string().contains(&encoded));
        mirror
            .hydrate(
                &json!({"id":"t","turns":[{"id":"r","status":"completed","items":[item]}]}),
                0,
            )
            .unwrap();
        let history = messages(mirror.thread("t").unwrap());
        assert_eq!(history[1]["id"], live[1]["id"]);
        assert_eq!(history[1]["nativeMedia"], live[1]["nativeMedia"]);
        item["failure"] = json!({"type":"usageLimitExceeded"});
        let mut mirror = central_agent_codex_runtime::mirror::Mirror::default();
        mirror
            .hydrate(
                &json!({"id":"t","turns":[{"id":"r","status":"failed","items":[item]}]}),
                0,
            )
            .unwrap();
        let failed = messages(mirror.thread("t").unwrap());
        assert_eq!(failed.len(), 1);
        assert!(!failed[0].to_string().contains(&encoded));
    }
}
