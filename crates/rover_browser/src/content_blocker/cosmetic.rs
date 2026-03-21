/// Cosmetic filtering script — CSS element hiding with MutationObserver guard.
///
/// Injected via WKUserScript at document-start so the `<style>` node arrives
/// before first paint. Selectors sourced from EasyList cosmetic filters and
/// uBOL's default lists.
pub(super) const COSMETIC_JS: &str = r#"(function(){
  'use strict';
  var STYLE_ID='rover-cosmetic-v1';
  var SELECTORS=[
    'ins.adsbygoogle','.adsbygoogle','[id^="google_ads_"]','[id^="google_ad_"]',
    '[id^="div-gpt-ad"]','[id*="google_ads_"]','[data-ad-slot]','[data-ad-client]',
    '[data-adunit]','[data-ad-unit]',
    '#ad','#ads','#ad-banner','#ad-container','#ad-slot','#ad-unit',
    '#advertisement','#advertisment','#ad-area','#ad-space','#ad-wrap',
    '#ad-top','#ad-bottom','#ad-left','#ad-right','#ad-sidebar',
    '#ad-leaderboard','#ad-skyscraper','#ad-rectangle','#ad-interstitial',
    '#ads-container','#ads-wrapper','#ads-area','#ads-box',
    '#banner-ad','#banner_ad','#leaderboard','#leaderboard-ad',
    '#billboard','#billboard-ad','#halfpage','#halfpage-ad',
    '#mrec','#mpu','#mpu-ad','#sidebar-ad','#top-ad','#bottom-ad',
    '#dfp-ad','#dfp-slot',
    '.ad','.ads','.ad-banner','.ad-container','.ad-slot','.ad-unit',
    '.ad-wrapper','.ad-area','.ad-block','.ad-box','.ad-cell',
    '.ad-frame','.ad-image','.ad-label','.ad-leaderboard','.ad-link',
    '.ad-notice','.ad-placement','.ad-region','.ad-section','.ad-space',
    '.ad-spot','.ad-tag','.ad-tile','.ad-zone','.ad-holder',
    '.advertisement','.advertisements','.advertisment',
    '.advert','.adverts','.advertorial',
    '.ads-container','.ads-wrapper','.ads-area','.ads-box',
    '.ad-top','.ad-bottom','.ad-sidebar','.ad-skyscraper',
    '.banner-ad','.leaderboard-ad','.billboard-ad','.halfpage-ad',
    '.mrec-ad','.mpu-ad','.top-ad','.bottom-ad',
    '.dfp-ad','.dfp-slot',
    '.sticky-ad','.fixed-ad','.floating-ad','.overlay-ad',
    '[class*="ad-banner"]','[class*="ad-slot"]','[class*="ad-unit"]',
    '[class*="AdSlot"]','[class*="AdBanner"]','[class*="AdUnit"]',
    '[class*="adSlot"]','[class*="adBanner"]','[class*="adUnit"]',
    '[id*="ad-slot"]','[id*="ad-unit"]','[id*="ad-banner"]',
    '[class*="Advertisement"]','[class*="advertisement"]',
    '[class*="Sponsored"]',
    '[aria-label*="Advertisement" i]','[aria-label*="Sponsored" i]',
    '[aria-label*="Ad choices" i]',
    '.sponsored','.sponsored-content','.sponsored-link','.sponsored-post',
    '.promoted','.promoted-story','.promoted-tweet','.promoted-content',
    '.native-ad','.native-advertising','.native-content',
    '.promo-ad','.promo-unit',
    '[class*="sponsored"]','[data-sponsored]',
    '[class*="NativeAd"]','[class*="native-ad"]',
    '[id*="taboola"]','[class*="taboola"]',
    '#taboola-below-article-thumbnails','#taboola-below-main-column-thumbnails',
    '.trc_rbox_container','.trc_rbox_div',
    '[id*="outbrain"]','[class*="outbrain"]','.OUTBRAIN',
    '.ob-widget','.ob-smartfeed-wrapper',
    '#rc-widget','[class*="rc-widget"]','.rc-col',
    '#carbonads','.carbon-ads','#carbon-ads',
    '.bsa_it_ad','#bsap_apf','#bsap_ad',
    '[data-testid="placementTracking"]','[data-promoted-tweet]',
    '[data-ad-preview]','._7jyg','._li',
    '[class*="ape-ad"]','[id*="ape-ad"]','.ape_ad_unit',
    '.ad-video','.video-ad','.preroll-ad','.midroll-ad',
    '#ad-overlay','.ad-overlay',
    '[class*="VideoAd"]','[class*="video-ad"]',
    '#cookie-consent-overlay','.cookie-consent-overlay'
  ].join(', ');
  function inject(){
    try{
      var root=document.head||document.documentElement;
      if(!root){return}
      var el=document.getElementById(STYLE_ID);
      if(!el){el=document.createElement('style');el.id=STYLE_ID;root.insertBefore(el,root.firstChild)}
      el.textContent=SELECTORS+'{display:none!important;visibility:hidden!important}';
    }catch(_){}
  }
  inject();
  if(document.readyState==='loading'){document.addEventListener('DOMContentLoaded',inject,{once:true})}
  try{
    var guardian=new MutationObserver(function(mutations){
      for(var i=0;i<mutations.length;i++){
        var removed=mutations[i].removedNodes;
        for(var j=0;j<removed.length;j++){if(removed[j].id===STYLE_ID){inject();return}}
      }
    });
    guardian.observe(document.documentElement,{childList:true,subtree:true});
  }catch(_){}
})()"#;
