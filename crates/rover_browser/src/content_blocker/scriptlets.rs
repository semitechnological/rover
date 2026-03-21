/// Ad-framework and tracker stubs — scriptlet injection.
///
/// Derived from uBOL's `web_accessible_resources/` scriptlets. Injected at
/// document-start so stubs are in place before any page scripts run. Only stubs
/// globals that are currently undefined to avoid interfering with scripts that
/// legitimately loaded.
pub(super) const SCRIPTLETS_JS: &str = r#"(function(){
  'use strict';
  function noop(){}
  function noopReturn(){return {}}
  function noopPromise(){return Promise.resolve()}

  // Google Analytics (Universal Analytics)
  if(typeof window.ga==='undefined'){
    window.ga=function ga(){var cb=arguments[arguments.length-1];if(typeof cb==='function'){cb()}};
    window.ga.create=noopReturn;window.ga.getAll=function(){return []};
    window.ga.remove=noop;window.ga.loaded=true;
  }
  if(typeof window._gaq==='undefined'){window._gaq=[]}
  if(Array.isArray(window._gaq)&&!window._gaq._patched){window._gaq.push=noop;window._gaq._patched=true}

  // Google Analytics 4 / gtag.js
  window.dataLayer=window.dataLayer||[];
  if(typeof window.gtag==='undefined'){
    window.gtag=function gtag(){
      if(arguments[0]==='event'&&typeof arguments[2]==='object'){
        var cb=arguments[2].event_callback;if(typeof cb==='function'){cb()}
      }
    };
  }

  // Google AdSense / AdManager (adsbygoogle)
  (function(){
    var existing=window.adsbygoogle;
    if(existing&&typeof existing.push==='function'&&existing._patched){return}
    var stub=[];stub.push=noop;stub._patched=true;
    window.adsbygoogle=stub;
  })();

  // Google IMA SDK (video ads)
  if(typeof window.google==='undefined'||typeof window.google.ima==='undefined'){
    var google=window.google=window.google||{};
    google.ima={
      AdDisplayContainer:function(){},
      AdError:function(m){this.message=m},
      AdErrorEvent:{Type:{AD_ERROR:'adError'}},
      AdEvent:{Type:{ALL_ADS_COMPLETED:'allAdsCompleted',CLICK:'click',COMPLETE:'complete',
        FIRST_QUARTILE:'firstQuartile',LOADED:'loaded',MIDPOINT:'midpoint',
        PAUSED:'pause',STARTED:'start',THIRD_QUARTILE:'thirdQuartile'}},
      AdsLoader:function(){this.addEventListener=noop;this.contentComplete=noop;
        this.destroy=noop;this.getSettings=noopReturn;this.requestAds=noop},
      AdsManager:{FREEWHEEL:'fw',VAST:'vast'},
      AdsManagerLoadedEvent:{Type:{ADS_MANAGER_LOADED:'adsManagerLoaded'}},
      AdsRequest:function(){},AdsRenderingSettings:function(){},
      CompanionAdSelectionSettings:function(){},
      ImaSdkSettings:function(){this.setLocale=noop;this.setPlayerType=noop;
        this.setPlayerVersion=noop;this.setVpaidAllowed=noop;this.setVpaidMode=noop;
        this.setDisableCustomPlaybackForIOS10Plus=noop},
      VERSION:'3.460.0'
    };
    google.ima.AdDisplayContainer.prototype={initialize:noop,destroy:noop};
  }

  // Amazon Advertising (apstag / aax)
  if(typeof window.apstag==='undefined'){
    window.apstag={init:noop,fetchBids:function(cfg,cb){if(typeof cb==='function'){cb([])}},
      setDisplayBids:noop,targetingKeys:function(){return []},_Q:[]};
  }
  if(typeof window.aax_write==='undefined'){window.aax_write=noop;window.aax_render_ad=noop}

  // Facebook Pixel (fbq)
  if(typeof window.fbq==='undefined'){
    var fbq=function(){};fbq.callMethod=noop;fbq.queue=[];fbq.push=fbq;
    fbq.loaded=true;fbq.version='2.0';window.fbq=fbq;window._fbq=fbq;
  }

  // Twitter/X Pixel (twq)
  if(typeof window.twq==='undefined'){
    window.twq=function(){};window.twq.exe=noop;window.twq.version='1.1';window.twq.queue=[];
  }

  // LinkedIn Insight Tag (lintrk)
  window._linkedin_data_partner_ids=window._linkedin_data_partner_ids||[];
  if(typeof window.lintrk==='undefined'){window.lintrk=noop}

  // Snapchat Pixel (snaptr)
  if(typeof window.snaptr==='undefined'){window.snaptr=noop}

  // TikTok Pixel (ttq)
  if(typeof window.ttq==='undefined'){
    window.ttq={load:noop,page:noop,track:noop,identify:noop,instances:[],
      debug:noop,on:noop,off:noop,once:noop,ready:noop,alias:noop,group:noop,
      enableCookie:noop,disableCookie:noop};
  }

  // Pinterest Pixel (pintrk)
  if(typeof window.pintrk==='undefined'){window.pintrk=noop}

  // Taboola (_taboola push queue)
  if(typeof window._taboola==='undefined'){window._taboola={push:noop}}
  else if(window._taboola&&!window._taboola._patched){window._taboola.push=noop;window._taboola._patched=true}

  // Outbrain (OBR)
  if(typeof window.OBR==='undefined'){
    window.OBR={extern:{renderWidget:noop,callWidget:noop,loadWidget:noop}};
  }

  // Hotjar (hj)
  if(typeof window.hj==='undefined'){
    var hjQueue=[];window.hj=function(){hjQueue.push(arguments)};
    window.hj.q=hjQueue;window._hjSettings=window._hjSettings||{};
  }

  // Criteo (criteo_q push queue)
  if(typeof window.Criteo==='undefined'){window.Criteo={events:{push:noop}}}
  window.criteo_q=window.criteo_q||[];
  if(!window.criteo_q._patched){window.criteo_q.push=noop;window.criteo_q._patched=true}

  // Anti-adblock decoy (nofab / nobab)
  try{
    var decoy=document.createElement('div');
    decoy.className='ad-slot pub_300x250 pub_300x250m pub_728x90 text-ad textAd text_ad text_ads';
    decoy.style.cssText='position:absolute;top:-999px;left:-999px;width:1px;height:1px;overflow:hidden;pointer-events:none;';
    decoy.setAttribute('aria-hidden','true');
    (document.body||document.documentElement).appendChild(decoy);
  }catch(_){}
})()"#;
