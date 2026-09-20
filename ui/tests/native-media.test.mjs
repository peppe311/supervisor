import test from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import os from "node:os";
import { compile } from "svelte/compiler";
import { render } from "svelte/server";
import { nativeMedia } from "../src/lib/native-media.ts";
import { finalOutputIndex, isNativeMedia } from "../src/lib/agent-timeline.ts";

test("native preview accepts only bounded inline raster descriptors", () => {
  const media={state:"ready",previewDataUrl:"data:image/png;base64,YWJj",width:8,height:6};
  assert.equal(nativeMedia(media).state,"ready");
  for (const value of ["https://example.invalid/secret","file:///C:/private.png","data:image/svg+xml;base64,YWJj","data:image/png;base64,YWJj\" onload=evil"])
    assert.equal(nativeMedia({...media,previewDataUrl:value}).state,"unavailable");
  for (const change of [{width:0},{height:NaN},{width:1.5},{width:20_000},{width:16_000,height:16_000}])
    assert.equal(nativeMedia({...media,...change}).state,"unavailable");
  assert.equal(nativeMedia(null),null);
  assert.equal(nativeMedia({state:"future"}),null);
  assert.deepEqual(nativeMedia({state:"unavailable",label:"Original kept",previewDataUrl:"https://private"}),{state:"unavailable",label:"Original kept"});
});

test("native media is distinct from the final answer and other providers", () => {
  const media={role:"assistant",kind:"native_media",provider:"codex_app_server",nativeMedia:{state:"unavailable",label:"Not embedded"}};
  assert.equal(isNativeMedia(media),true);
  assert.equal(isNativeMedia({...media,provider:"claude"}),false);
  assert.equal(finalOutputIndex([media]),-1);
  assert.equal(finalOutputIndex([{role:"assistant",kind:"message",messagePhase:"final_answer",text:"Done"},media]),0);
});

test("actual retained native image renders in the compiled shared component", {skip:!process.env.CENTRAL_AGENT_TEST_NATIVE_IMAGE_DIR}, async () => {
  const directory=fs.realpathSync(process.env.CENTRAL_AGENT_TEST_NATIVE_IMAGE_DIR);
  const relative=path.relative(fs.realpathSync(os.tmpdir()),directory);
  assert.ok(relative && !relative.startsWith('..') && !path.isAbsolute(relative));
  assert.ok(path.basename(directory).startsWith('central-native-image-'));
  const row=JSON.parse(fs.readFileSync(path.join(directory,'native-image-row.json'),'utf8'));
  assert.equal(isNativeMedia(row),true);
  const value=nativeMedia(row.nativeMedia);
  assert.equal(value.state,'ready');
  const source=fs.readFileSync(new URL('../src/components/NativeMedia.svelte',import.meta.url),'utf8');
  let code=compile(source,{filename:'NativeMedia.svelte',generate:'server'}).js.code;
  code=code.replace(/from (["'])([^"']+)\1/g,(whole,quote,specifier)=>`from ${JSON.stringify(specifier==='../lib/native-media'?new URL('../src/lib/native-media.ts',import.meta.url).href:import.meta.resolve(specifier))}`);
  const {default:Component}=await import(`data:text/javascript;base64,${Buffer.from(code).toString('base64')}`);
  const html=render(Component,{props:{value}}).body;
  // Assertions deliberately avoid printing the large original image or HTML.
  assert.equal((html.match(/<img\b/g)||[]).length,2);
  assert.ok(html.includes(`src="${value.previewDataUrl}"`));
  assert.ok(html.includes(`width="${value.width}"`) && html.includes(`height="${value.height}"`));
  assert.ok(html.includes('Enlarge generated image') && html.includes('Close preview'));
  assert.ok(!/<dialog[^>]*\sopen(?:\s|>)/.test(html));
  assert.ok(!html.includes('Inline image unavailable'));
  assert.ok(!html.includes('file://') && !html.includes('https://'));
});
