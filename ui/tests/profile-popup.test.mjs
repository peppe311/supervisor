import test from 'node:test';
import assert from 'node:assert/strict';
import {placeProfilePopup} from '../src/lib/profile-popup.ts';

test('profile popup uses the empty gutter without crossing Explorer or transcript content',()=>{
  for(const width of [1920,2560,3840]) {
    const conversation={left:240,top:0,right:width,bottom:1080};
    const contentLeft=240+(width-240-900)/2;
    const anchor={left:contentLeft-68,right:contentLeft-16,top:960,bottom:1012};
    const result=placeProfilePopup({anchor,conversation,viewport:{left:0,top:0,right:width,bottom:1080},contentLeft,width:288,height:300,gap:8});
    assert.equal(result.hasGutter,true);
    assert.ok(result.left>=conversation.left+8);
    assert.ok(result.left+result.width<=contentLeft-8);
    assert.ok(result.top+Math.min(300,result.maxHeight)<=anchor.top-8);
  }
});

test('narrow, short and offset viewports keep a scrollable popup inside the available chat area',()=>{
  for(const [width,height] of [[480,360],[560,720],[720,480]]) {
    for(const offset of [0,24]) {
      const viewport={left:offset,top:offset,right:width+offset,bottom:height+offset};
      const conversation={left:177+offset,top:offset,right:width+offset,bottom:height+offset};
      for(const anchorTop of [offset+12,height+offset-72]) {
        const anchor={left:190+offset,right:242+offset,top:anchorTop,bottom:anchorTop+52};
        const result=placeProfilePopup({anchor,conversation,viewport,contentLeft:245+offset,width:288,height:600,gap:8});
        assert.equal(result.hasGutter,false);
        assert.ok(result.width>0 && result.maxHeight>0);
        assert.ok(result.left>=conversation.left+8 && result.left+result.width<=viewport.right-8);
        assert.ok(result.top>=viewport.top+8 && result.top+Math.min(600,result.maxHeight)<=viewport.bottom-8);
        assert.equal(result.side,anchorTop===offset+12?'below':'above');
      }
    }
  }
});
