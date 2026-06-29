function openLink(u){window.open(u,'_blank','noopener');}
(function(){
  var FOLDERS=["House","Techno","Disco/Edits","Ambient","Breakbeat"],OUTF=["MP3 320","AIFF","WAV"];
  var T=[
    {a:"Mr. Fingers",t:"Can You Feel It",lbl:"Trax",yr:1986,fmt:"AIFF · 16",dur:"7:48",fake:false,file:"mr fingers - cyfi (org).aiff",real:"conforme",gen:"Deep House"},
    {a:"Larry Heard",t:"Mystery of Love",lbl:"Alleviated",yr:1985,fmt:"MP3 320",dur:"6:12",fake:true,file:"larry_heard_mystery_320.mp3",real:"~128 kbps (transcodé)",gen:"Deep House"},
    {a:"Chez Damier",t:"Can You Feel It",lbl:"KMS",yr:1992,fmt:"AIFF · 16",dur:"7:02",fake:false,file:"chez damier cyfi.aif",real:"conforme",gen:"Deep House"},
    {a:"Moodymann",t:"Shades of Jae",lbl:"KDJ",yr:1997,fmt:"WAV · 24",dur:"5:30",fake:false,file:"01_shades_of_jae.wav",real:"conforme",gen:"Deep House"},
    {a:"Theo Parrish",t:"Falling Up",lbl:"Peacefrog",yr:1998,fmt:"AIFF · 16",dur:"9:20",fake:false,file:"theo parrish-falling up.aiff",real:"conforme",gen:"Detroit House"},
    {a:"M. Jefferson",t:"Move Your Body",lbl:"Trax",yr:1986,fmt:"MP3 320",dur:"6:30",fake:false,file:"move your body (320).mp3",real:"conforme",gen:"Chicago House"},
    {a:"Robert Owens",t:"Bring Down the Walls",lbl:"Trax",yr:1986,fmt:"AIFF · 16",dur:"7:30",fake:false,file:"bring down the walls.aiff",real:"conforme",gen:"Chicago House"},
    {a:"Lil Louis",t:"French Kiss",lbl:"FFRR",yr:1989,fmt:"WAV · 24",dur:"9:55",fake:false,file:"french kiss FULL.wav",real:"conforme",gen:"Acid House"},
    {a:"Octave One",t:"Blackwater",lbl:"430 West",yr:2002,fmt:"AIFF · 16",dur:"6:45",fake:false,file:"octave one - blackwater.aiff",real:"conforme",gen:"Detroit Techno"}
  ];
  T[1].status="resource";T[1].ecartReason="fake";T[1].storeAvail=["beatport","traxsource","juno"];
  T[4].status="resource";T[4].ecartReason="truncated";T[4].storeAvail=["juno","bandcamp"];
  T[0].duplicate=true;T[0].dupOf={title:"Mr. Fingers — Can You Feel It",fmt:"AIFF · 16",path:"House/mr fingers - cyfi (org).aiff"};
  T.forEach(function(x,i){if(!x.status)x.status="pending";x.folder=null;
    x.wave=[];for(var k=0;k<32;k++){var h=18+Math.round(Math.abs(Math.sin((k+i*3)*0.7))*60+((k*7+i*13)%15));x.wave.push(h>96?96:h);}
    x.spec=[];for(var m=0;m<32;m++){var b=56+((m*5+i*11)%40);if(x.fake&&m>22)b=10+((m*3)%14);x.spec.push(b);}});
  var LIB=[["Mr. Fingers — Can You Feel It","AIFF","120","7:48","30521"],["Chez Damier — Can You Feel It","AIFF","122","7:02","41822"],["Marshall Jefferson — Move Your Body","MP3","118","6:30","18743"],["Lil Louis — French Kiss","WAV","120","9:55","9210"],["Robert Owens — Bring Down the Walls","AIFF","121","7:30","55190"],["Fingers Inc. — Distant Planet","AIFF","119","8:10","73004"]];
  var view="home",cur=0,playing=false,tempo=0,selFolder=0,playPos=0,bibPlaying=-1,bibPos=0.3,creating=false,rkbSynced=false,outFmt=null,revMode="detail",midTab="ecoute",sel={},queueShowAll=false,dupScanDone=false,dupDismissed={},timeMode="elapsed",qw=180,bibHL=-1;
  var content=document.getElementById('content'),nav=document.getElementById('nav');
  function extOf(f){return f==="MP3 320"?"mp3":(f==="WAV"?"wav":"aiff");}
  function defFmt(i){if(i<0)return "AIFF";return /AIFF|WAV/.test(T[i].fmt)?"AIFF":"MP3 320";}
  function pT(s){var p=s.split(':');return (+p[0])*60+(+p[1]);}function fT(sec){var m=Math.floor(sec/60),s=Math.round(sec%60);return m+':'+(s<10?'0':'')+s;}
  function np(from){for(var i=from;i<T.length;i++)if(T[i].status==="pending")return i;for(var j=0;j<T.length;j++)if(T[j].status==="pending")return j;return -1;}
  function fileTo(fi){if(view!=="revue"||cur<0)return;selFolder=fi;T[cur].status="filed";T[cur].folder=fi;rkbSynced=false;playPos=0;outFmt=null;midTab="ecoute";cur=np(cur+1);render();}
  function jeter(){if(view!=="revue"||cur<0)return;T[cur].status=T[cur].fake?"resource":"trash";playPos=0;outFmt=null;midTab="ecoute";cur=np(cur+1);render();}
  function cnt(s){return T.filter(function(x){return x.status===s;}).length;}
  function byFolder(){var b={};T.forEach(function(x){if(x.status==="filed")b[x.folder]=(b[x.folder]||0)+1;});return b;}
  function nSel(){var n=0;for(var k in sel)if(sel[k])n++;return n;}
  function block(){content.style.display="block";content.style.padding="14px 18px";content.style.overflowY="auto";}
  function render(){Array.prototype.forEach.call(nav.querySelectorAll('.nv'),function(n){n.classList.toggle('on',n.dataset.view===view);});
    if(view==="revue")return revMode==="batch"?renderBatch():renderRevue();
    if(view==="home")return renderHome();if(view==="biblio")return renderBiblio();if(view==="rkb")return renderRkb();if(view==="cle")return renderCle();if(view==="ecarts")return renderEcarts();if(view==="journal")return renderJournal();return renderReglages();}

  function renderHome(){
    content.style.display="flex";content.style.padding="14px 18px";content.style.overflowY="auto";content.style.flexDirection="column";
    var filed=cnt("filed"),res=cnt("resource"),tr=cnt("trash"),pend=cnt("pending");
    var fakes=T.filter(function(x){return x.status==="resource"&&x.ecartReason==="fake";}).length;
    var noMeta=T.filter(function(x){return x.status==="filed"&&!x.lbl;}).length;
    var byF=byFolder(),mx=Math.max(1,filed);
    var bars=FOLDERS.map(function(f,i){var n=byF[i]||0;return n?'<div style="display:flex;align-items:center;gap:8px;margin-bottom:5px"><span style="width:90px;flex:none;font-size:11px;color:var(--color-text-secondary);white-space:nowrap;overflow:hidden;text-overflow:ellipsis">'+f+'</span><span style="flex:1;height:7px;background:var(--color-background-secondary);border-radius:4px"><span style="display:block;height:100%;border-radius:4px;background:var(--color-text-info);width:'+Math.round(n/mx*100)+'%"></span></span><span style="width:18px;text-align:right;font-size:11px;color:var(--color-text-tertiary)">'+n+'</span></div>':'';}).join('');
    var statCard=function(label,val,color,vw,sub){
      var style='cursor:'+(vw?'pointer':'default')+';background:var(--color-background-secondary);border-radius:var(--border-radius-md);padding:9px 12px'+(vw?';border:0.5px solid var(--color-border-tertiary)':'');
      return '<div class="mc" '+(vw?'data-view="'+vw+'"':'')+' style="'+style+'">'
        +'<div class="l">'+label+(vw?' <i class="ti ti-arrow-right" style="font-size:10px;opacity:.5"></i>':'')+'</div>'
        +'<div class="n" style="color:'+color+'">'+val+'</div>'
        +(sub?'<div style="font-size:10px;color:var(--color-text-tertiary);margin-top:2px">'+sub+'</div>':'')
        +'</div>';
    };
    var pendBanner=pend?'<div style="display:flex;align-items:center;justify-content:space-between;background:var(--color-background-info);border-radius:var(--border-radius-md);padding:10px 14px;margin-bottom:12px"><div><div style="font-size:13px;font-weight:500;color:var(--color-text-info)">'+pend+' fichiers à trier</div><div style="font-size:11px;color:var(--color-text-info);opacity:.8">dont '+fakes+' faux détecté'+(fakes>1?'s':'')+'</div></div><button data-view="revue">Trier <i class="ti ti-arrow-right" style="font-size:12px;vertical-align:-2px"></i></button></div>':'';
    var dossiers='<div class="col-h" style="margin-top:12px">Dossiers surveillés</div>'
      +'<div class="srow"><span class="v"><i class="ti ti-folder"></i> ~/Downloads/soulseek</span><span style="display:flex;align-items:center;gap:9px"><span style="font-size:11px;color:var(--color-text-info)">8 nouveaux</span><span class="tog"></span></span></div>'
      +'<div class="srow"><span class="v"><i class="ti ti-folder"></i> ~/Downloads/promos</span><span style="display:flex;align-items:center;gap:9px"><span style="font-size:11px;color:var(--color-text-info)">1 nouveau</span><span class="tog"></span></span></div>'
      +'<div style="margin:8px 0 0"><button><i class="ti ti-plus" style="font-size:13px;vertical-align:-2px"></i> ajouter un dossier</button></div>';
    var leftHtml='<div class="h1">Accueil</div>'
      +'<div style="display:grid;grid-template-columns:repeat(2,1fr);gap:8px;margin-bottom:12px">'
        +statCard('Rangés',filed,'var(--color-text-success)','biblio','dans la bibliothèque')
        +statCard('À re-sourcer',res,'var(--color-text-danger)','ecarts',fakes+' faux · '+(res-fakes)+' tronqués')
        +statCard('Corbeille',tr,'var(--color-text-tertiary)','',null)
        +statCard('Sans métadonnées',noMeta+12,'var(--color-text-warning)','','tags incomplets')
      +'</div>'
      +pendBanner
      +dossiers
      +(filed?'<div class="col-h" style="margin-top:12px">Répartition par dossier</div>'+bars:'');
    content.innerHTML='<div class="home-body"><div class="home-left">'+leftHtml+'</div></div>';
    if(window.__siftHome)window.__siftHome();
  }

  function renderRevue(){
    content.style.display="flex";content.style.padding="0";content.style.flexDirection="";content.style.overflowY="";
    var pendingCount=cnt("pending"),doneCount=T.length-pendingCount;
    content.innerHTML='<div class="queue" id="qcol" style="width:'+qw+'px"><div style="display:flex;align-items:center;justify-content:space-between;margin-bottom:3px"><span class="col-h" style="margin:0">Queue</span><span style="display:flex;gap:3px"><span data-act="revmode" data-m="detail" title="Vue détail" style="cursor:pointer;color:var(--color-text-info)"><i class="ti ti-layout-list" style="font-size:14px"></i></span><span data-act="revmode" data-m="batch" title="Mode batch" style="cursor:pointer;color:var(--color-text-tertiary)"><i class="ti ti-table" style="font-size:14px"></i></span></span></div><div class="pbar"><div class="pfill" id="pf" style="width:0%"></div></div><div id="ql"></div>'+(doneCount?'<div style="padding:5px 4px 0"><span data-act="togglequeue" style="font-size:10px;color:var(--color-text-tertiary);cursor:pointer;text-decoration:underline">'+(queueShowAll?'Masquer les traités':'+ '+doneCount+' traités')+'</span></div>':'')+'</div><div class="qdrag" id="qdrag" title="Drag to widen the list"></div><div class="mid" id="mid"></div><div class="dest"><div class="col-h">Destination</div><div id="fldz"></div><div id="filfoot" style="margin-top:14px"></div></div>';
    var done2=T.length-pendingCount;document.getElementById('pf').style.width=Math.round(done2/T.length*100)+"%";
    var h="";T.forEach(function(x,i){
      var isPending=x.status==="pending";
      if(!isPending&&!queueShowAll&&i!==cur)return;
      var ic="ti-circle",col="",cls="qi";
      if(i===cur)cls+=" cur",ic="ti-player-play";else if(x.status==="filed")cls+=" done",ic="ti-check";
      else if(x.status==="resource")cls+=" done",ic="ti-x",col="color:var(--color-text-danger)";
      else if(x.status==="trash")cls+=" done",ic="ti-trash",col="color:var(--color-text-tertiary)";
      else if(x.fake)ic="ti-alert-triangle",col="color:var(--color-text-danger)";
      var slskBtn=(x.status==="resource"||x.fake)?'<button data-act="qslsk" data-i="'+i+'" data-txt="'+encodeURIComponent(x.a+' '+x.t)+'" title="Copier nom Soulseek" style="flex:none;width:18px;height:18px;padding:0;border:none;display:inline-flex;align-items:center;justify-content:center;color:var(--color-text-tertiary);background:transparent" onclick="event.stopPropagation()"><i class="ti ti-copy" style="font-size:11px"></i></button>':'';
      var dupBadge=x.duplicate?'<i class="ti ti-copy" style="font-size:11px;flex:none;color:var(--color-text-secondary)" title="Doublon — déjà en biblio"></i>':'';
      h+='<div class="'+cls+'" data-act="sel" data-i="'+i+'"><i class="ti '+ic+'" style="'+col+'"></i><span style="flex:1;min-width:0;overflow:hidden;text-overflow:ellipsis">'+x.a+' — '+x.t+'</span>'+dupBadge+slskBtn+'</div>';});
    document.getElementById('ql').innerHTML=h;
    var fh="";FOLDERS.forEach(function(f,i){fh+='<div class="fld'+(i===selFolder?' on':'')+'" data-act="file" data-i="'+i+'"><span class="kbd">'+(i+1)+'</span> '+f+'</div>';});
    fh+= creating ? '<input id="newin" placeholder="nom du dossier…" style="width:100%;font-size:12px;padding:5px 7px;margin-top:2px">' : '<div class="fld" data-act="newfld" style="color:var(--color-text-tertiary)"><i class="ti ti-plus" style="font-size:14px"></i> nouveau</div>';
    document.getElementById('fldz').innerHTML=fh;renderMid();
    if(creating){var ni=document.getElementById('newin');if(ni)ni.focus();}
    var qd=document.getElementById('qdrag');
    qd.addEventListener('mousedown',function(ev){ev.preventDefault();var x0=ev.clientX,w0=qw;
      function mv(e2){qw=Math.min(380,Math.max(140,w0+(e2.clientX-x0)));document.getElementById('qcol').style.width=qw+'px';}
      function up(){document.removeEventListener('mousemove',mv);document.removeEventListener('mouseup',up);}
      document.addEventListener('mousemove',mv);document.addEventListener('mouseup',up);});
    if(window.__siftQueue)window.__siftQueue();
  }

  function renderMid(){var mid=document.getElementById('mid');if(!mid)return;
    if(cur<0){mid.innerHTML='<div style="flex:1;display:flex;flex-direction:column;align-items:center;justify-content:center;text-align:center;gap:6px;color:var(--color-text-secondary)"><i class="ti ti-circle-check" style="font-size:26px;color:var(--color-text-success)"></i><div style="font-size:13px;font-weight:500;color:var(--color-text-primary)">Tout est trié</div><div style="font-size:11px">'+cnt("filed")+' rangés · '+cnt("resource")+' re-sourcer · '+cnt("trash")+' corbeille</div><button data-view="rkb" style="margin-top:4px">Mettre à jour Rekordbox <i class="ti ti-playlist" style="font-size:12px;vertical-align:-2px"></i></button></div>';return;}
    if(outFmt===null)outFmt=defFmt(cur);
    var x=T[cur];
    var vb=x.fake?'background:var(--color-background-danger);color:var(--color-text-danger)':x.duplicate?'background:var(--color-background-secondary);color:var(--color-text-secondary)':'background:var(--color-background-success);color:var(--color-text-success)';
    var vt=x.fake?'<i class="ti ti-alert-triangle" style="font-size:12px"></i> faux':x.duplicate?'<i class="ti ti-copy" style="font-size:12px"></i> doublon':'<i class="ti ti-shield-check" style="font-size:12px"></i> vrai';
    // header: cover + title + verdict badge
    var head='<div style="font-size:10px;color:var(--color-text-tertiary);font-family:var(--font-mono);white-space:nowrap;overflow:hidden;text-overflow:ellipsis;margin-bottom:6px"><i class="ti ti-file" style="font-size:11px;vertical-align:-1px"></i> '+x.file+'</div>'
      +'<div style="display:flex;align-items:center;gap:8px;margin-bottom:10px"><div class="cov"><i class="ti ti-disc"></i></div><div style="min-width:0;flex:1"><div style="font-weight:500;font-size:13px;white-space:nowrap;overflow:hidden;text-overflow:ellipsis">'+x.a+' — '+x.t+'</div><div style="color:var(--color-text-tertiary);font-size:11px">'+x.lbl+' · '+x.yr+'</div></div><span style="flex:none;display:inline-flex;align-items:center;gap:4px;'+vb+';padding:3px 7px;border-radius:var(--border-radius-md);font-size:11px;font-weight:500">'+vt+'</span></div>';
    var dupBanner=x.duplicate?'<div style="display:flex;align-items:center;gap:8px;background:var(--color-background-secondary);border-radius:var(--border-radius-md);padding:8px 11px;margin-bottom:10px;font-size:11px"><i class="ti ti-copy" style="font-size:14px;flex:none;color:var(--color-text-secondary)"></i><div style="min-width:0"><div style="font-weight:500;color:var(--color-text-primary)">Déjà en bibliothèque</div><div style="color:var(--color-text-tertiary);white-space:nowrap;overflow:hidden;text-overflow:ellipsis">'+x.dupOf.path+' · '+x.dupOf.fmt+'</div></div><button data-act="dupvoir" style="flex:none;font-size:11px">Voir</button></div>':'';
    var wave="";x.wave.forEach(function(hh,k){wave+='<span style="height:'+hh+'%;background:'+(k/32<=playPos?'var(--color-text-primary)':'var(--color-text-tertiary)')+'"></span>';});
    // 1. LECTEUR — sous le titre
    var dur=pT(x.dur),elapsed=Math.round(playPos*dur),remaining=dur-elapsed;
    var bpm=120+(cur*3)%9;
    var timeDisp=timeMode==="elapsed"?fT(elapsed):'-'+fT(remaining);
    var player='<div style="display:flex;align-items:center;gap:8px;margin-bottom:11px;padding:8px 10px;background:var(--color-background-secondary);border-radius:var(--border-radius-md)">'
      +'<div style="flex:none;display:flex;flex-direction:column;align-items:center;gap:2px"><button data-act="play" aria-label="Lecture (Espace)" title="Espace = lecture/pause" style="width:28px;height:28px;border-radius:50%;display:inline-flex;align-items:center;justify-content:center;padding:0"><i class="ti '+(playing?'ti-player-pause':'ti-player-play')+'" style="font-size:13px"></i></button><span class="kbd" style="font-size:8px">espace</span></div>'
      +'<div class="bars" data-act="seek" style="flex:1">'+wave+'</div>'
      +'<div style="flex:none;display:flex;flex-direction:column;align-items:center;gap:2px">'
        +'<span data-act="timemodetog" id="tdisp" title="Écoulé ⇄ restant" style="font-family:var(--font-mono);font-size:10px;color:var(--color-text-secondary);cursor:pointer;min-width:36px;text-align:center;border-bottom:1px dotted var(--color-border-secondary)">'+timeDisp+'</span>'
        +'<span style="font-family:var(--font-mono);font-size:9px;color:var(--color-text-tertiary);text-align:center">'+bpm+' bpm</span>'
      +'</div>'
      +'<div style="flex:none;display:flex;flex-direction:column;align-items:center"><input type="range" min="-8" max="8" step="1" value="'+tempo+'" data-act="tempo" aria-label="Tempo" style="writing-mode:vertical-lr;direction:rtl;width:15px;height:30px"><span id="tout" style="font-family:var(--font-mono);font-size:9px">'+(tempo>0?'+':'')+tempo+'%</span></div>'
      +'</div>';
    var realPill=x.fake?'<span class="pill" style="background:var(--color-background-danger);color:var(--color-text-danger)"><i class="ti ti-alert-triangle" style="font-size:10px"></i> '+x.real+'</span>':'<span class="pill" style="background:var(--color-background-success);color:var(--color-text-success)"><i class="ti ti-check" style="font-size:10px"></i> conforme</span>';
    var specCap=x.fake?'<span style="color:var(--color-text-danger)"><i class="ti ti-alert-triangle" style="font-size:9px;vertical-align:-1px"></i> coupure nette ~16 kHz = transcodé</span>':"énergie jusqu'en haut = encodage conforme";
    // 2. ENCODAGE + SPECTROGRAMME
    var encoding='<div style="display:flex;align-items:center;gap:6px;margin-bottom:6px;font-size:11px;flex-wrap:wrap"><span style="color:var(--color-text-tertiary)">Encodage</span><span class="pill">'+x.fmt+'</span><i class="ti ti-arrow-right" style="font-size:12px;color:var(--color-text-tertiary)"></i>'+realPill+'<span class="pill">'+x.dur+'</span></div>'
      +'<canvas id="spc" width="400" height="46" style="width:100%;height:46px;border-radius:4px;display:block"></canvas>'
      +'<div style="font-size:9px;color:var(--color-text-tertiary);margin:2px 0 9px">'+specCap+'</div>';
    // 3. FORMAT SORTIE
    var chips=OUTF.map(function(f){return '<span class="chip'+(outFmt===f?' on':'')+'" data-act="fmtsel" data-f="'+f+'">'+f+'</span>';}).join(' ');
    var nm=x.a+' - '+x.t+' (Original Mix) ['+x.lbl+'].'+extOf(outFmt);
    var sortir='<div style="padding-top:8px;border-top:0.5px solid var(--color-border-tertiary)"><div style="display:flex;align-items:center;gap:6px;flex-wrap:wrap;margin-bottom:5px"><span style="font-size:10px;color:var(--color-text-tertiary)">Sortir en</span>'+chips+'</div><div style="font-size:10px;color:var(--color-text-tertiary);word-break:break-all;line-height:1.5">→ <span style="font-family:var(--font-mono);color:var(--color-text-secondary)">'+nm+'</span></div></div>';
    var meta='<div style="margin-top:10px;padding-top:9px;border-top:0.5px solid var(--color-border-tertiary)"><div style="display:flex;align-items:center;gap:5px;margin-bottom:6px"><span style="font-size:9px;letter-spacing:.04em;text-transform:uppercase;color:var(--color-text-tertiary)">Métadonnées</span><span class="pill" style="background:var(--color-background-info);color:var(--color-text-info);font-size:9px;padding:1px 6px"><i class="ti ti-download" style="font-size:9px"></i> pullé de Discogs</span></div>'
      +'<div style="display:grid;grid-template-columns:auto 1fr auto 1fr;gap:3px 8px;font-size:11px;align-items:center">'
      +'<span style="color:var(--color-text-tertiary)">Label</span><span>'+x.lbl+'</span><span style="color:var(--color-text-tertiary)">Année</span><span>'+x.yr+'</span>'
      +'<span style="color:var(--color-text-tertiary)">Genre</span><span>'+x.gen+'</span><span style="color:var(--color-text-tertiary)">BPM</span><span>'+(120+(cur*3)%9)+'</span></div>'
      +'<div style="display:flex;align-items:center;gap:4px;flex-wrap:wrap;margin-top:6px"><span style="color:var(--color-text-tertiary);font-size:11px">Tags</span><span class="pill" style="font-size:10px">peak-time</span><span class="pill" style="font-size:10px">classic</span><span class="pill" style="font-size:10px;color:var(--color-text-tertiary)"><i class="ti ti-plus" style="font-size:9px"></i></span></div></div>';
    var jBtn=x.fake
      ?'<button data-act="jeter" style="color:var(--color-text-warning)" title="Fichier faux — ira dans Écartés pour re-sourcer"><span class="kbd">X</span> ⚠ Re-sourcer</button>'
      :x.duplicate
      ?'<button data-act="jeter" style="color:var(--color-text-danger)" title="Envoyer dans Écartés"><span class="kbd">X</span> Écarter (doublon)</button>'
      :'<button data-act="jeter" style="color:var(--color-text-danger)" title="Envoyer dans Écartés"><span class="kbd">X</span> Écarter</button>';
    mid.innerHTML='<div class="mid-scroll">'+head+dupBanner+player+encoding+sortir+meta+'</div>'
      +'<div style="flex:none;padding-top:10px;display:flex;gap:8px;border-top:0.5px solid var(--color-border-tertiary)"><button data-act="commit" style="flex:1;background:var(--color-background-info);color:var(--color-text-info);border:none;font-weight:500"><span class="kbd">&crarr;</span> Ranger &rarr; '+FOLDERS[selFolder]+'</button>'+jBtn+'</div>';
    if(midTab==="ecoute"&&cur>=0)drawSpec(x);
  }

  function drawSpec(x){var c=document.getElementById('spc');if(!c||!c.getContext)return;var ctx=c.getContext('2d'),W=c.width,H=c.height;
    var seed=0;for(var s=0;s<x.file.length;s++)seed+=x.file.charCodeAt(s);
    function rnd(n){var v=Math.sin(n*12.9898+seed)*43758.5453;return v-Math.floor(v);}
    function col(e){e=e<0?0:e>1?1:e;var st=[[20,12,38],[58,18,92],[181,23,158],[255,179,71],[255,222,128]];var t=e*(st.length-1),i=Math.floor(t),f=t-i;if(i>=st.length-1){i=st.length-2;f=1;}var a=st[i],b=st[i+1];return 'rgb('+Math.round(a[0]+(b[0]-a[0])*f)+','+Math.round(a[1]+(b[1]-a[1])*f)+','+Math.round(a[2]+(b[2]-a[2])*f)+')';}
    var cutoff=x.fake?0.60:1.0;ctx.fillStyle='rgb(20,12,38)';ctx.fillRect(0,0,W,H);
    for(var i=0;i<W;i++){var beat=0.6+0.4*Math.abs(Math.sin(i*0.09+seed));
      for(var j=0;j<H;j++){var fy=1-j/H;var e=Math.pow(1-fy,0.55)*beat;
        e*=0.6+0.4*Math.sin(i*0.18+fy*7+seed);e*=0.65+0.35*rnd(i*131+j*7);
        if(fy>cutoff)e*=Math.max(0,1-(fy-cutoff)*16);
        if(e>0.04){ctx.fillStyle=col(e);ctx.fillRect(i,j,1,1);}}}
    if(x.fake){var y=Math.round(H*(1-cutoff));ctx.strokeStyle='rgba(255,90,90,0.9)';ctx.setLineDash([4,3]);ctx.beginPath();ctx.moveTo(0,y);ctx.lineTo(W,y);ctx.stroke();ctx.setLineDash([]);}
  }

  function renderBatch(){block();var pend=[];T.forEach(function(x,i){if(x.status==="pending")pend.push(i);});var n=nSel();
    var rows=pend.map(function(i){var x=T[i];var on=!!sel[i];return '<div class="br" data-act="bcheck" data-i="'+i+'"><span class="cbx'+(on?' on':'')+'">'+(on?'<i class="ti ti-check" style="font-size:11px"></i>':'')+'</span><span style="flex:1;min-width:0;white-space:nowrap;overflow:hidden;text-overflow:ellipsis">'+x.a+' — '+x.t+'</span>'+(x.fake?'<span class="pill" style="background:var(--color-background-danger);color:var(--color-text-danger);flex:none">faux</span>':'<span class="pill" style="background:var(--color-background-success);color:var(--color-text-success);flex:none">vrai</span>')+'<span class="pill" style="flex:none">'+x.fmt+'</span></div>';}).join('');
    var dchips=FOLDERS.map(function(f,i){return '<span class="chip'+(i===selFolder?' on':'')+'" data-act="file2" data-i="'+i+'">'+f+'</span>';}).join(' ');
    content.innerHTML='<div style="display:flex;align-items:center;justify-content:space-between;margin-bottom:10px"><span class="h1" style="margin:0">À traiter — mode batch</span><button data-act="revmode" data-m="detail"><i class="ti ti-layout-list" style="font-size:13px;vertical-align:-2px"></i> vue détail</button></div>'
      +(rows||'<div style="font-size:12px;color:var(--color-text-tertiary)">File vide.</div>')
      +'<div style="margin-top:14px;display:flex;align-items:center;gap:8px;flex-wrap:wrap;padding-top:11px;border-top:0.5px solid var(--color-border-secondary)"><span style="font-size:11px;color:var(--color-text-tertiary)">Ranger dans :</span>'+dchips+'</div>'
      +'<div style="margin-top:11px;display:flex;gap:8px"><button data-act="brange"'+(n?'':' disabled')+'>Ranger la sélection ('+n+')</button><button data-act="bjeter" style="color:var(--color-text-danger)"'+(n?'':' disabled')+'>Écarter ('+n+')</button></div>';
  }

  function renderRkb(){block();var filed=cnt("filed"),byF=byFolder();
    var pls=FOLDERS.map(function(f,i){var n=byF[i]||0;return '<div class="srow"><span class="v"><i class="ti ti-playlist"></i> '+f+'</span><span style="font-size:11px;color:'+(n?'var(--color-text-info)':'var(--color-text-tertiary)')+'">'+(n?'+ '+n:'à jour')+'</span></div>';}).join('');
    var action= rkbSynced?'<div style="display:flex;align-items:center;gap:8px;background:var(--color-background-success);border-radius:var(--border-radius-md);padding:12px 15px;margin-bottom:15px;color:var(--color-text-success)"><i class="ti ti-circle-check" style="font-size:18px"></i><span style="font-size:13px;font-weight:500">Rekordbox à jour — '+filed+' synchronisés</span></div>':'<div style="display:flex;align-items:center;justify-content:space-between;background:var(--color-background-info);border-radius:var(--border-radius-md);padding:12px 15px;margin-bottom:15px"><div><div style="font-size:14px;font-weight:500;color:var(--color-text-info)">'+filed+' rangés à pousser</div><div style="font-size:11px;color:var(--color-text-info);opacity:.8">dernière sync : il y a 2 j</div></div><button data-act="rksync">Mettre à jour <i class="ti ti-refresh" style="font-size:12px;vertical-align:-2px"></i></button></div>';
    content.innerHTML='<div class="h1">Rekordbox</div>'+action+'<div class="col-h">Playlists générées</div>'+pls+'<div class="col-h" style="margin-top:14px">Mode</div><div style="display:flex;gap:8px;margin-bottom:12px"><span class="chip on">XML — sûr</span><span class="chip">master.db — natif ⚠️</span></div><div style="display:flex;gap:8px;align-items:flex-start;font-size:11px;color:var(--color-text-warning);background:var(--color-background-warning);border-radius:var(--border-radius-md);padding:9px 12px"><i class="ti ti-alert-triangle" style="font-size:14px;flex:none"></i><span>Ferme Rekordbox avant de synchroniser. En master.db : backup auto.</span></div>';
  }

  function renderBiblio(){block();
    var rows=LIB.map(function(r,i){var on=i===bibPlaying;var hl=i===bibHL;return '<div class="lr"'+(on?' style="background:var(--color-background-info);border-radius:var(--border-radius-md);border-bottom:none"':hl?' style="background:var(--color-background-warning);border-radius:var(--border-radius-md);outline:1px solid var(--color-text-warning)"':'')+'><button class="pb" data-act="bplay" data-i="'+i+'" aria-label="Écouter"'+(on?' style="color:var(--color-text-info)"':'')+'><i class="ti '+(on?'ti-player-pause':'ti-player-play')+'" style="font-size:12px"></i></button><span style="flex:1;min-width:0;white-space:nowrap;overflow:hidden;text-overflow:ellipsis'+(on?';color:var(--color-text-info);font-weight:500':'')+'">'+r[0]+'</span><span class="pill" style="flex:none">'+r[1]+'</span><span style="flex:none;width:28px;text-align:right;font-family:var(--font-mono);color:var(--color-text-tertiary)">'+r[2]+'</span><span style="flex:none;width:34px;text-align:right;font-family:var(--font-mono);color:var(--color-text-tertiary)">'+r[3]+'</span><button class="lk" data-act="link" data-i="'+i+'" aria-label="Fiche Discogs"><i class="ti ti-external-link" style="font-size:13px;color:var(--color-text-tertiary)"></i></button></div>';}).join('');
    var player="";if(bibPlaying>=0){var r=LIB[bibPlaying];var tot=pT(r[3]);var w="";for(var k=0;k<40;k++){var hh=18+Math.round(Math.abs(Math.sin((k+bibPlaying*3)*0.7))*60+((k*7)%15));if(hh>96)hh=96;w+='<span style="height:'+hh+'%;background:'+(k/40<=bibPos?'var(--color-text-info)':'var(--color-text-tertiary)')+'"></span>';}player='<div style="margin-top:10px;background:var(--color-background-secondary);border-radius:var(--border-radius-md);padding:9px 11px;display:flex;align-items:center;gap:10px"><button class="pb" data-act="bplay" data-i="'+bibPlaying+'" aria-label="Pause" style="color:var(--color-text-info)"><i class="ti ti-player-pause" style="font-size:13px"></i></button><span style="flex:none;width:116px;min-width:0;font-size:11px;font-weight:500;white-space:nowrap;overflow:hidden;text-overflow:ellipsis">'+r[0]+'</span><div class="bars" data-act="bseek" style="flex:1;height:30px">'+w+'</div><span style="flex:none;font-family:var(--font-mono);font-size:10px;color:var(--color-text-tertiary)">'+fT(tot*bibPos)+' / '+r[3]+'</span></div>';}

    // scanner doublons
    var DUP_GROUPS=[
      {id:0,tracks:[
        {label:'Mr. Fingers — Can You Feel It',fmt:'AIFF · 16',path:'House/mr fingers - cyfi (org).aiff',bpm:'120',dur:'7:48',src:'biblio'},
        {label:'Mr. Fingers — Can You Feel It',fmt:'AIFF · 16',path:'House/mr fingers can you feel it.aiff',bpm:'120',dur:'7:48',src:'biblio'}
      ]},
    ];
    var scanSection='';
    if(!dupScanDone){
      scanSection='<div style="display:flex;align-items:center;justify-content:space-between;background:var(--color-background-secondary);border-radius:var(--border-radius-md);padding:9px 13px;margin-bottom:13px"><div style="font-size:12px;color:var(--color-text-secondary)"><i class="ti ti-arrows-shuffle" style="font-size:13px;vertical-align:-2px"></i> Scanner les doublons internes</div><button data-act="dupscan">Lancer <i class="ti ti-search" style="font-size:11px;vertical-align:-1px"></i></button></div>';
    } else {
      var groups=DUP_GROUPS.filter(function(g){return !dupDismissed[g.id];});
      if(groups.length===0){
        scanSection='<div style="display:flex;align-items:center;gap:8px;background:var(--color-background-success);border-radius:var(--border-radius-md);padding:9px 13px;margin-bottom:13px;font-size:12px;color:var(--color-text-success)"><i class="ti ti-circle-check" style="font-size:14px"></i> Aucun doublon — bibliothèque propre</div>';
      } else {
        var gHtml=groups.map(function(g){
          return '<div style="background:var(--color-background-secondary);border-radius:var(--border-radius-md);padding:9px 12px;margin-bottom:8px">'
            +'<div style="display:flex;align-items:center;gap:6px;margin-bottom:8px"><i class="ti ti-copy" style="font-size:13px;color:var(--color-text-secondary)"></i><span style="font-size:11px;font-weight:500">'+g.tracks[0].label+'</span><span style="font-size:10px;color:var(--color-text-tertiary)">· similarité 100%</span></div>'
            +g.tracks.map(function(tr,ti){return '<div style="display:flex;align-items:center;gap:7px;padding:5px 7px;background:var(--color-background-primary);border-radius:var(--border-radius-md);margin-bottom:4px;font-size:11px">'
              +'<i class="ti ti-file" style="color:var(--color-text-tertiary);font-size:12px;flex:none"></i>'
              +'<span style="flex:1;min-width:0;font-family:var(--font-mono);font-size:10px;overflow:hidden;text-overflow:ellipsis;white-space:nowrap">'+tr.path+'</span>'
              +'<span class="pill" style="flex:none">'+tr.fmt+'</span>'
              +'<span style="flex:none;font-family:var(--font-mono);font-size:10px;color:var(--color-text-tertiary)">'+tr.dur+'</span>'
              +'<button data-act="dupkeep" data-gid="'+g.id+'" data-ti="'+ti+'" style="font-size:10px;padding:2px 7px;color:var(--color-text-success)">Garder</button>'
              +'<button data-act="dupjeter" data-gid="'+g.id+'" data-ti="'+ti+'" style="font-size:10px;padding:2px 7px;color:var(--color-text-danger)">Jeter</button>'
              +'</div>';}).join('')
            +'</div>';
        }).join('');
        scanSection='<div style="margin-bottom:13px"><div style="display:flex;align-items:center;justify-content:space-between;margin-bottom:8px"><span style="font-size:12px;font-weight:500"><i class="ti ti-copy" style="font-size:13px;vertical-align:-2px;color:var(--color-text-secondary)"></i> '+groups.length+' doublon'+(groups.length>1?'s':'')+' détecté'+(groups.length>1?'s':'')+' dans la bibliothèque</span></div>'+gHtml+'</div>';
      }
    }

    content.innerHTML='<div style="display:flex;align-items:center;gap:8px;margin-bottom:10px"><div style="flex:1;display:flex;align-items:center;gap:7px;border:0.5px solid var(--color-border-secondary);border-radius:var(--border-radius-md);padding:6px 10px;color:var(--color-text-tertiary)"><i class="ti ti-search" style="font-size:14px"></i><span style="font-size:12px">Rechercher…</span></div><span class="chip on">Tous</span><span class="chip">Lossless</span><span class="chip">MP3</span></div>'
      +scanSection
      +'<div style="display:flex;gap:14px"><div style="width:130px;flex:none"><div class="col-h">Dossiers</div>'+['House 412','Techno 318','Disco/Edits 196','Ambient 142','Breakbeat 98'].map(function(s,i){var p=s.split(' '),n=p.pop();return '<div class="fld'+(i===0?' on':'')+'" style="justify-content:space-between"><span>'+p.join(' ')+'</span><span style="font-size:11px;opacity:.7">'+n+'</span></div>';}).join('')+'</div>'
      +'<div style="flex:1;min-width:0"><div style="display:flex;justify-content:space-between;margin-bottom:5px"><span style="font-size:13px;font-weight:500">House</span><span style="font-size:11px;color:var(--color-text-tertiary)">412 morceaux</span></div>'+rows+player+'</div></div>';
    if(window.__siftBiblio)window.__siftBiblio();
  }

  var STORES=[
    {id:'beatport',label:'Beatport',url:function(q){return 'https://www.beatport.com/search?q='+q;}},
    {id:'traxsource',label:'Traxsource',url:function(q){return 'https://www.traxsource.com/search?term='+q;}},
    {id:'juno',label:'Juno',url:function(q){return 'https://www.junodownload.com/search/?q[all][]='+q;}},
    {id:'bandcamp',label:'Bandcamp',url:function(q){return 'https://bandcamp.com/search?q='+q;}},
    {id:'amazon',label:'Amazon',url:function(q){return 'https://www.amazon.fr/s?k='+q+'&i=digital-music';}},
    {id:'apple',label:'Apple Music',url:function(q){return 'https://music.apple.com/fr/search?term='+q;}}
  ];

  function renderJournal(){block();content.innerHTML='';if(window.__siftJournal)window.__siftJournal();}

  function renderEcarts(){block();
    var ecarts=T.filter(function(x){return x.status==="resource"||x.status==="trash";});
    var filterR=ecarts.filter(function(x){return x.status==="resource";});
    var filterT=ecarts.filter(function(x){return x.status==="trash";});
    function slsk(x){return x.a+' '+x.t;}
    function reasonLabel(x){
      if(x.ecartReason==="truncated")return '<span class="pill" style="background:var(--color-background-warning);color:var(--color-text-warning);flex:none"><i class="ti ti-cut" style="font-size:9px"></i> tronqué</span>';
      if(x.ecartReason==="duplicate")return '<span class="pill" style="background:var(--color-background-secondary);color:var(--color-text-secondary);flex:none"><i class="ti ti-copy" style="font-size:9px"></i> doublon</span>';
      return '<span class="pill" style="background:var(--color-background-danger);color:var(--color-text-danger);flex:none"><i class="ti ti-alert-triangle" style="font-size:9px"></i> faux</span>';
    }
    var rows=ecarts.map(function(x){
      var qi=T.indexOf(x);
      var isRes=x.status==="resource";
      var q=encodeURIComponent(slsk(x));
      var avail=x.storeAvail||[];
      var storeLinks=isRes?STORES.filter(function(s){return avail.length===0||avail.indexOf(s.id)>=0;}).map(function(s){return '<a data-act="estore" data-i="'+qi+'" data-url="'+encodeURIComponent(s.url(q))+'" style="font-size:10px;color:var(--color-text-info);cursor:pointer;text-decoration:none;white-space:nowrap">'+s.label+'</a>';}).join('<span style="color:var(--color-border-secondary);margin:0 3px">·</span>'):'';
      var slskTxt=encodeURIComponent(x.a+' '+x.t);
      return '<div style="padding:7px 4px;border-bottom:0.5px solid var(--color-border-tertiary)">'
        +'<div style="display:flex;align-items:center;gap:7px">'
        +'<div style="flex:1;min-width:0"><div style="white-space:nowrap;overflow:hidden;text-overflow:ellipsis;font-size:12px;font-weight:500">'+x.a+' — '+x.t+'</div>'
        +'<div style="font-size:10px;color:var(--color-text-tertiary);font-family:var(--font-mono);white-space:nowrap;overflow:hidden;text-overflow:ellipsis">'+x.file+'</div></div>'
        +reasonLabel(x)
        +'<button class="lk" data-act="etrash" data-i="'+qi+'" title="Corbeille"><i class="ti ti-trash" style="font-size:12px;color:var(--color-text-tertiary)"></i></button>'
        +'</div>'
        +(isRes?'<div style="margin-top:5px;display:flex;flex-wrap:wrap;align-items:center;gap:4px"><button data-act="eslsk" data-txt="'+slskTxt+'" style="font-size:10px;padding:2px 7px;color:var(--color-text-secondary)"><i class="ti ti-copy" style="font-size:10px;vertical-align:-1px"></i> Slsk</button>'+(storeLinks?'<span style="color:var(--color-border-secondary)">·</span>'+storeLinks:'')+'</div>':'')
        +'</div>';
    }).join('');
    content.innerHTML='<div class="h1">Écartés</div>'
      +'<div style="display:flex;gap:7px;margin-bottom:12px;flex-wrap:wrap">'
      +'<span class="pill" style="background:var(--color-background-danger);color:var(--color-text-danger)"><i class="ti ti-alert-circle" style="font-size:10px"></i> '+filterR.length+' à re-sourcer</span>'
      +'<span class="pill"><i class="ti ti-trash" style="font-size:10px"></i> '+filterT.length+' en attente corbeille</span>'
      +(filterT.length?'<button data-act="evider" style="font-size:10px;padding:2px 8px;color:var(--color-text-danger)">Vider la corbeille ('+filterT.length+')</button>':'')
      +'</div>'
      +(rows||'<div style="font-size:12px;color:var(--color-text-tertiary)">Aucun fichier écarté.</div>')
      ;
    if(window.__siftEcarts)window.__siftEcarts();
  }

  function renderCle(){block();
    content.innerHTML='<div class="h1">Formater la clé</div><div style="display:flex;gap:8px;align-items:flex-start;background:var(--color-background-warning);border-radius:var(--border-radius-md);padding:9px 12px;margin-bottom:14px;font-size:11px;color:var(--color-text-warning)"><i class="ti ti-alert-triangle" style="font-size:15px;flex:none"></i><span>Volumes <strong>amovibles uniquement</strong> — le formatage <strong>efface tout</strong>.</span></div><div class="col-h">Volume</div><div class="srow"><span class="v"><i class="ti ti-usb"></i> USB DJ — 28 Go <span style="color:var(--color-text-tertiary)">(FAT32)</span></span><i class="ti ti-circle-check" style="color:var(--color-text-info);font-size:16px"></i></div><div class="srow"><span class="v"><i class="ti ti-usb"></i> SSD Samsung T7 — 500 Go <span style="color:var(--color-text-tertiary)">(exFAT)</span></span><i class="ti ti-circle" style="color:var(--color-text-tertiary);font-size:16px"></i></div><div class="col-h" style="margin-top:14px">Format</div><div style="display:flex;gap:8px;margin-bottom:14px"><span class="chip on">FAT32 — compat tous CDJ</span><span class="chip">exFAT — CDJ récents</span></div><div style="display:flex;align-items:center;gap:9px"><div style="flex:1;border:0.5px solid var(--color-border-secondary);border-radius:var(--border-radius-md);padding:6px 10px;font-size:12px;color:var(--color-text-tertiary)">tape « USB DJ » pour confirmer</div><button style="color:var(--color-text-danger);border-color:var(--color-border-danger)">Formater</button></div>';
  }

  function renderReglages(){block();var rows=[['Dossiers source','2 dossiers'],['Dossiers destination','6 genres'],['Format lossless','AIFF · 16-bit / 44,1 kHz'],['Format lossy','MP3 320 (pas d\'upscale)'],['Modèle de nommage','Artiste - Titre (Mix) [Label]'],['Sensibilité anti-fake','standard'],['Identification','tags → Discogs → manuel'],['Discogs','connecté'],['Intégration Rekordbox','XML (master.db désactivé)'],['Normalisation','désactivée']];
    content.innerHTML='<div class="h1">Réglages</div>'+rows.map(function(r){return '<div class="srow"><span>'+r[0]+'</span><span class="v">'+r[1]+' <i class="ti ti-chevron-right" style="font-size:14px;color:var(--color-text-tertiary)"></i></span></div>';}).join('');
    if(window.__siftReglages)window.__siftReglages();
  }

  var pa=document.getElementById('pa');
  pa.addEventListener('click',function(e){
    var v=e.target.closest('[data-view]');if(v){view=v.dataset.view;creating=false;if(view!=='biblio')bibHL=-1;render();return;}
    var el=e.target.closest('[data-act]');if(!el)return;var act=el.dataset.act;
    if(act==='sel'){cur=+el.dataset.i;playPos=0;outFmt=null;midTab="ecoute";render();}
    else if(act==='play'){playing=!playing;renderMid();}
    else if(act==='timemodetog'){timeMode=timeMode==="elapsed"?"remaining":"elapsed";renderMid();}
    else if(act==='file'){selFolder=+el.dataset.i;renderRevue();}
    else if(act==='file2'){selFolder=+el.dataset.i;renderBatch();}
    else if(act==='commit'){fileTo(selFolder);}
    else if(act==='jeter'){jeter();}
    else if(act==='newfld'){creating=true;render();}
    else if(act==='fmtsel'){outFmt=el.dataset.f;renderMid();}
    else if(act==='mtab'){midTab=el.dataset.m;renderMid();}
    else if(act==='revmode'){revMode=el.dataset.m;sel={};render();}
    else if(act==='togglequeue'){queueShowAll=!queueShowAll;renderRevue();}
    else if(act==='bcheck'){var bi=+el.dataset.i;sel[bi]=!sel[bi];renderBatch();}
    else if(act==='brange'){for(var k in sel){if(sel[k]&&T[k].status==="pending"){T[k].status="filed";T[k].folder=selFolder;}}sel={};rkbSynced=false;render();}
    else if(act==='bjeter'){for(var k2 in sel){if(sel[k2]&&T[k2].status==="pending")T[k2].status=T[k2].fake?"resource":"trash";}sel={};render();}
    else if(act==='rksync'){rkbSynced=true;render();}
    else if(act==='dupscan'){dupScanDone=true;renderBiblio();}
    else if(act==='dupvoir'){bibHL=0;view='biblio';render();}
    else if(act==='evider'){T.forEach(function(x){if(x.status==="trash")x.status="purged";});renderEcarts();}
    else if(act==='dupkeep'||act==='dupjeter'){dupDismissed[+el.dataset.gid]=true;renderBiblio();}
    else if(act==='etrash'){T[+el.dataset.i].status="trash";renderEcarts();}
    else if(act==='estore'){openLink(decodeURIComponent(el.dataset.url));}
    else if(act==='qslsk'){var txt=decodeURIComponent(el.dataset.txt);navigator.clipboard.writeText(txt).catch(function(){});var prev=el.innerHTML;el.innerHTML='<i class="ti ti-check" style="font-size:11px"></i>';setTimeout(function(){el.innerHTML=prev;},1200);}
    else if(act==='eslsk'){navigator.clipboard.writeText(decodeURIComponent(el.dataset.txt)).catch(function(){});el.innerHTML='<i class="ti ti-check" style="font-size:12px;vertical-align:-2px"></i> Copié';setTimeout(function(){renderEcarts();},1400);}
    else if(act==='bplay'){var i=+el.dataset.i;if(bibPlaying===i){bibPlaying=-1;}else{bibPlaying=i;bibPos=0.3;}renderBiblio();}
    else if(act==='bseek'){var r2=el.getBoundingClientRect();bibPos=Math.min(1,Math.max(0,(e.clientX-r2.left)/r2.width));renderBiblio();}
    else if(act==='link'){var parts=LIB[+el.dataset.i][0].split(/\s*[—–-]\s*/);var q=parts.length>1?parts[0]+' '+parts[1]:parts[0];openLink('https://www.discogs.com/search/?type=release&q='+encodeURIComponent(q));}
    else if(act==='seek'){var r=el.getBoundingClientRect();playPos=Math.min(1,Math.max(0,(e.clientX-r.left)/r.width));renderMid();}
  });
  pa.addEventListener('input',function(e){if(e.target.dataset.act==='tempo'){tempo=parseInt(e.target.value,10);var o=document.getElementById('tout');if(o)o.textContent=(tempo>0?'+':'')+tempo+'%';}});
  pa.addEventListener('keydown',function(e){
    // Live (Tauri): the real keyboard layer (installFilingKeys) owns SPACE/Enter/X/1-9. This
    // mockup handler is a web-demo vestige — if it ran here, its renderMid() would repaint the
    // demo data (Mr. Fingers) over the real track. Same inTauri test as main.ts. Hors Tauri
    // (démo web Vercel) il reste actif.
    if('__TAURI_INTERNALS__' in window)return;
    if(e.target.id==='newin'){if(e.key==='Enter'){var val=e.target.value.trim();if(val){FOLDERS.push(val);creating=false;fileTo(FOLDERS.length-1);}}else if(e.key==='Escape'){creating=false;render();}return;}
    if(e.target.tagName==='INPUT')return;if(view!=="revue"||revMode!=="detail")return;
    var nk=parseInt(e.key,10);
    if(nk>=1&&nk<=Math.min(9,FOLDERS.length)){selFolder=nk-1;renderRevue();}else if(e.key==='Enter'){e.preventDefault();fileTo(selFolder);}else if(e.key==='x'||e.key==='X')jeter();else if(e.key===' '){e.preventDefault();playing=!playing;renderMid();}
  });
  render();
})();
