// Executed only by --check-ui-startup in hidden synthetic WebViews. No pixels
// are captured, no Codex connection is started and no files/URLs are opened.
try {
  const host=document.createElement('div');document.body.append(host);
  const canvas=document.createElement('canvas');canvas.width=8;canvas.height=6;
  const value={state:'ready',label:'Generated image',previewDataUrl:canvas.toDataURL('image/png'),width:8,height:6};
  const flush=async()=>{await Promise.resolve();await Promise.resolve();};
  window.CentralAgentSvelte.renderNativeMedia(host,value);await flush();
  const image=host.querySelector('img');
  if(!image)throw new Error('Native image component did not mount');
  await image.decode();
  if(image.naturalWidth!==8 || image.naturalHeight!==6)throw new Error('Native image dimensions changed');
  host.querySelector('[aria-label="Enlarge generated image"]').click();await flush();
  const dialog=host.querySelector('dialog');
  if(!dialog.open)throw new Error('Native image enlargement failed');
  window.CentralAgentSvelte.renderNativeMedia(host,{...value});await flush();
  if(!dialog.open || host.querySelector('img')!==image)throw new Error('Native stream update reset the image or its dialog');
  dialog.querySelector('button').click();await flush();
  if(dialog.open)throw new Error('Native preview did not close');
  window.CentralAgentSvelte.renderNativeMedia(host,{...value,previewDataUrl:'https://example.invalid/private.png'});await flush();
  if(host.querySelector('img') || !host.textContent.includes('unavailable'))throw new Error('Native preview started an external fetch or omitted the fallback');
  window.CentralAgentSvelte.renderNativeMedia(host,value);await flush();
  expectedNativeMediaError=new Event('error');
  host.querySelector('img').dispatchEvent(expectedNativeMediaError);
  expectedNativeMediaError=null;await flush();
  if(host.querySelector('img') || !host.textContent.includes('could not be decoded'))throw new Error('Native decoder error is not explicit');
  host.remove();await flush();
} catch(error) {startupErrors.push(String(error));}
