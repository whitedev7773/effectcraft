(function () {
  // Run with the source project open, then use File > Save As > Save a Copy As XML.
  // The data is stored in temporary project folders because file writes may be disabled.
  var PREFIX = "EC_EXPORT_V1_";
  var MAX_PROPERTIES = 300000;
  var seenProperties = 0;
  function get(fn, fallback) { try { return fn(); } catch (e) { return fallback; } }
  function finite(n) { return typeof n === "number" && isFinite(n) ? n : null; }
  function characterStyle(r) {
    return {font:get(function(){return r.font},""), fontStyle:get(function(){return r.fontStyle},""),
      fontSize:get(function(){return r.fontSize},null),
      fillColor:get(function(){return r.fillColor},null), strokeColor:get(function(){return r.strokeColor},null),
      applyFill:get(function(){return r.applyFill},true), applyStroke:get(function(){return r.applyStroke},false),
      strokeWidth:get(function(){return r.strokeWidth},0), tracking:get(function(){return r.tracking},0),
      leading:get(function(){return r.leading},0), autoLeading:get(function(){return r.autoLeading},true),
      baselineShift:get(function(){return r.baselineShift},0), fauxBold:get(function(){return r.fauxBold},false),
      fauxItalic:get(function(){return r.fauxItalic},false), allCaps:get(function(){return r.allCaps},false),
      smallCaps:get(function(){return r.smallCaps},false)};
  }
  function characterRuns(v) {
    var t=get(function(){return v.text},"");
    if (!v.characterRange || t.length > 10000) return null;
    var out=[], prior="";
    for (var i=0;i<t.length;) {
      var end=i+1, first=t.charCodeAt(i), next=t.charCodeAt(end);
      if (first>=0xD800 && first<=0xDBFF && next>=0xDC00 && next<=0xDFFF) end++;
      var range=get(function(){return v.characterRange(i,end)},null);
      if (!range) return null;
      var style=characterStyle(range), part=t.substring(i,end);
      var signature=json(style);
      if (out.length && signature===prior) out[out.length-1].text+=part;
      else out.push({text:part,style:style});
      prior=signature;
      i=end;
    }
    return out;
  }
  function data(v, depth) {
    if (depth > 10 || v === null || v === undefined) return null;
    var t = typeof v;
    if (t === "string" || t === "boolean") return v;
    if (t === "number") return finite(v);
    if (v instanceof Array) {
      var a = [];
      for (var i = 0; i < v.length; i++) a.push(data(v[i], depth + 1));
      return a;
    }
    if (typeof TextDocument !== "undefined" && v instanceof TextDocument) {
      return {kind:"text", text:get(function(){return v.text},""), font:get(function(){return v.font},""),
        fontSize:get(function(){return v.fontSize},null), fillColor:data(get(function(){return v.fillColor},null),depth+1),
        strokeColor:data(get(function(){return v.strokeColor},null),depth+1), applyFill:get(function(){return v.applyFill},true),
        applyStroke:get(function(){return v.applyStroke},false), strokeWidth:get(function(){return v.strokeWidth},0),
        tracking:get(function(){return v.tracking},0), leading:get(function(){return v.leading},0),
        autoLeading:get(function(){return v.autoLeading},true), justification:get(function(){return v.justification},null),
        baselineShift:get(function(){return v.baselineShift},0), fauxBold:get(function(){return v.fauxBold},false),
        fauxItalic:get(function(){return v.fauxItalic},false), allCaps:get(function(){return v.allCaps},false),
        smallCaps:get(function(){return v.smallCaps},false), characterRuns:characterRuns(v)};
    }
    if (typeof Shape !== "undefined" && v instanceof Shape) {
      return {kind:"shape", vertices:data(v.vertices,depth+1), inTangents:data(v.inTangents,depth+1),
        outTangents:data(v.outTangents,depth+1), closed:v.closed};
    }
    if (typeof MarkerValue !== "undefined" && v instanceof MarkerValue) {
      return {kind:"marker",comment:get(function(){return v.comment},""),chapter:get(function(){return v.chapter},""),
        duration:get(function(){return v.duration},0),label:get(function(){return v.label},0),
        cuePointName:get(function(){return v.cuePointName},"")};
    }
    return {kind:"unsupported",className:get(function(){return v.reflect.name},"")};
  }
  function ease(arr) {
    var a=[]; if (!arr) return a;
    for (var i=0;i<arr.length;i++) a.push({speed:finite(arr[i].speed),influence:finite(arr[i].influence)});
    return a;
  }
  function key(p,i) {
    return {time:get(function(){return p.keyTime(i)},null),value:data(get(function(){return p.keyValue(i)},null),0),
      inInterpolation:get(function(){return p.keyInInterpolationType(i)},null),
      outInterpolation:get(function(){return p.keyOutInterpolationType(i)},null),
      inEase:ease(get(function(){return p.keyInTemporalEase(i)},[])),
      outEase:ease(get(function(){return p.keyOutTemporalEase(i)},[])),
      temporalContinuous:get(function(){return p.keyTemporalContinuous(i)},false),
      temporalAutoBezier:get(function(){return p.keyTemporalAutoBezier(i)},false),
      inSpatialTangent:data(get(function(){return p.keyInSpatialTangent(i)},null),0),
      outSpatialTangent:data(get(function(){return p.keyOutSpatialTangent(i)},null),0),
      spatialContinuous:get(function(){return p.keySpatialContinuous(i)},false),
      spatialAutoBezier:get(function(){return p.keySpatialAutoBezier(i)},false)};
  }
  function maskModeName(mode) {
    if (typeof MaskMode === "undefined") return null;
    var names = ["NONE", "ADD", "SUBTRACT", "INTERSECT", "LIGHTEN", "DARKEN", "DIFFERENCE"];
    for (var i=0;i<names.length;i++) if (mode === MaskMode[names[i]]) return names[i];
    return null;
  }
  function properties(group,depth) {
    if (depth > 32) throw new Error("property tree too deep");
    var out=[]; var n=get(function(){return group.numProperties},0);
    for (var i=1;i<=n;i++) {
      seenProperties++; if (seenProperties > MAX_PROPERTIES) throw new Error("too many properties");
      var p=get(function(){return group.property(i)},null); if (!p) continue;
      var q={name:get(function(){return p.name},""),matchName:get(function(){return p.matchName},""),
        propertyType:get(function(){return p.propertyType},null),propertyValueType:get(function(){return p.propertyValueType},null),
        enabled:get(function(){return p.enabled},true)};
      if (q.matchName === "ADBE Mask Atom") {
        q.maskMode=maskModeName(get(function(){return p.maskMode},null));
        q.inverted=get(function(){return p.inverted},null);
      }
      if (get(function(){return p.propertyType},null)===PropertyType.PROPERTY) {
        q.value=data(get(function(){return p.value},null),0);
        q.expression=get(function(){return p.expression},"");
        q.expressionEnabled=get(function(){return p.expressionEnabled},false);
        q.dimensionsSeparated=get(function(){return p.dimensionsSeparated},false);
        q.keys=[];
        var nk=get(function(){return p.numKeys},0);
        for (var k=1;k<=nk;k++) q.keys.push(key(p,k));
      } else q.children=properties(p,depth+1);
      out.push(q);
    }
    return out;
  }
  function layer(l) {
    return {index:l.index,id:get(function(){return l.id},null),name:l.name,
      type:get(function(){return l instanceof TextLayer?"Text":l instanceof ShapeLayer?"Shape":l instanceof CameraLayer?"Camera":l instanceof LightLayer?"Light":l.nullLayer?"Null":"AV"},"AV"),
      sourceId:get(function(){return l.source.id},null),parentId:get(function(){return l.parent.id},null),
      startTime:get(function(){return l.startTime},0),inPoint:get(function(){return l.inPoint},0),
      outPoint:get(function(){return l.outPoint},0),stretch:get(function(){return l.stretch},100),
      enabled:get(function(){return l.enabled},true),audioEnabled:get(function(){return l.audioEnabled},false),
      threeDLayer:get(function(){return l.threeDLayer},false),adjustmentLayer:get(function(){return l.adjustmentLayer},false),
      shy:get(function(){return l.shy},false),solo:get(function(){return l.solo},false),locked:get(function(){return l.locked},false),
      motionBlur:get(function(){return l.motionBlur},false),collapseTransformation:get(function(){return l.collapseTransformation},false),
      timeRemapEnabled:get(function(){return l.timeRemapEnabled},false),label:get(function(){return l.label},0),
      blendingMode:get(function(){return l.blendingMode},null),trackMatteType:get(function(){return l.trackMatteType},null),
      hasTrackMatte:get(function(){return l.hasTrackMatte},false),
      trackMatteLayerId:get(function(){return l.trackMatteLayer?l.trackMatteLayer.id:null},null),
      properties:properties(l,0)};
  }
  function item(x) {
    var q={index:x.index,id:get(function(){return x.id},null),name:x.name,
      parentId:get(function(){return x.parentFolder.id},null),comment:get(function(){return x.comment},"")};
    if (x instanceof CompItem) {
      q.type="Comp"; q.width=x.width;q.height=x.height;q.duration=x.duration;
      q.frameRate=x.frameRate;q.pixelAspect=x.pixelAspect;q.bgColor=data(x.bgColor,0);
      q.workAreaStart=x.workAreaStart;q.workAreaDuration=x.workAreaDuration;
      q.displayStartTime=get(function(){return x.displayStartTime},0);
      q.layers=[];for(var i=1;i<=x.numLayers;i++)q.layers.push(layer(x.layer(i)));
    } else if (x instanceof FootageItem) {
      q.type="Footage";q.width=x.width;q.height=x.height;q.duration=x.duration;
      q.frameRate=x.frameRate;q.pixelAspect=x.pixelAspect;
      q.file=get(function(){return x.file.fsName},null);
      q.solidColor=data(get(function(){return x.mainSource.color},null),0);
    } else q.type="Folder";
    return q;
  }
  function quote(s) {
    return '"'+String(s).replace(/\\/g,"\\\\").replace(/"/g,'\\"')
      .replace(/\r/g,"\\r").replace(/\n/g,"\\n").replace(/\t/g,"\\t")+'"';
  }
  function json(v) {
    if(v===null||v===undefined)return "null";
    if(typeof v==="string")return quote(v);
    if(typeof v==="number")return isFinite(v)?String(v):"null";
    if(typeof v==="boolean")return v?"true":"false";
    if(v instanceof Array){var a=[];for(var i=0;i<v.length;i++)a.push(json(v[i]));return "["+a.join(",")+"]";}
    var a=[];for(var k in v)if(v.hasOwnProperty(k))a.push(quote(k)+":"+json(v[k]));
    return "{"+a.join(",")+"}";
  }
  function put(name,comment){var f=app.project.items.addFolder(name);f.comment=comment;return f;}
  try {
    var p=app.project;
    var dump={format:"effectcraft-ae-observation-v1",source:get(function(){return p.file.fsName},null),
      itemCount:0,items:[]};
    var originalCount=p.numItems;
    for(var i=1;i<=originalCount;i++) {
      var originalItem=p.item(i);
      if (originalItem.name.indexOf("EC_EXPORT_")===0) continue;
      dump.items.push(item(originalItem));
    }
    dump.itemCount=dump.items.length;
    var encoded=json(dump), chunkSize=5000, count=Math.ceil(encoded.length/chunkSize);
    put(PREFIX+"META",json({length:encoded.length,chunks:count,items:dump.itemCount,properties:seenProperties}));
    for(var j=0;j<count;j++)put(PREFIX+("00000"+j).slice(-5),encoded.substring(j*chunkSize,(j+1)*chunkSize));
    put(PREFIX+"DONE",String(count));
  } catch(e) {put(PREFIX+"ERROR",String(e)+" line="+e.line+" properties="+seenProperties);}
})();
