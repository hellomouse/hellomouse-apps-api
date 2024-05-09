use sanitize_html::sanitize_str;
use sanitize_html::rules::{ Rules, Element };
use sanitize_html::rules::pattern::Pattern;
use regex::Regex;

pub fn get_html_rules() -> Rules {
    // Allowed tags
    let p = Element::new("p").attribute("style", Pattern::any()).attribute("class", Pattern::any());
    let ul = Element::new("ol").attribute("style", Pattern::any()).attribute("class", Pattern::any());
    let ol = Element::new("ul").attribute("style", Pattern::any()).attribute("class", Pattern::any());
    let li = Element::new("li").attribute("style", Pattern::any()).attribute("class", Pattern::any());

    // Formatting
    let b = Element::new("b").attribute("style", Pattern::any()).attribute("class", Pattern::any());
    let strong = Element::new("strong").attribute("style", Pattern::any()).attribute("class", Pattern::any());
    let i = Element::new("i").attribute("style", Pattern::any()).attribute("class", Pattern::any());
    let em = Element::new("em").attribute("style", Pattern::any()).attribute("class", Pattern::any());
    let mark = Element::new("mark").attribute("style", Pattern::any()).attribute("class", Pattern::any());
    let small = Element::new("small").attribute("style", Pattern::any()).attribute("class", Pattern::any());
    let del = Element::new("del").attribute("style", Pattern::any()).attribute("class", Pattern::any());
    let ins = Element::new("ins").attribute("style", Pattern::any()).attribute("class", Pattern::any());
    let sub = Element::new("sub").attribute("style", Pattern::any()).attribute("class", Pattern::any());
    let sup = Element::new("sup").attribute("style", Pattern::any()).attribute("class", Pattern::any());
    let br = Element::new("br").attribute("style", Pattern::any()).attribute("class", Pattern::any());

    let h1 = Element::new("h1").attribute("style", Pattern::any()).attribute("class", Pattern::any());
    let h2 = Element::new("h2").attribute("style", Pattern::any()).attribute("class", Pattern::any());
    let h3 = Element::new("h3").attribute("style", Pattern::any()).attribute("class", Pattern::any());
    let h4 = Element::new("h4").attribute("style", Pattern::any()).attribute("class", Pattern::any());
    let h5 = Element::new("h5").attribute("style", Pattern::any()).attribute("class", Pattern::any());

    let span = Element::new("span").attribute("style", Pattern::any()).attribute("class", Pattern::any());
    let div = Element::new("div").attribute("style", Pattern::any()).attribute("class", Pattern::any());
    let code = Element::new("code").attribute("style", Pattern::any()).attribute("class", Pattern::any());
    let pre = Element::new("pre").attribute("style", Pattern::any()).attribute("class", Pattern::any());

    let table = Element::new("table").attribute("style", Pattern::any()).attribute("class", Pattern::any());
    let tr = Element::new("tr").attribute("style", Pattern::any()).attribute("class", Pattern::any());
    let td = Element::new("td").attribute("style", Pattern::any()).attribute("class", Pattern::any());
    let thead = Element::new("thead").attribute("style", Pattern::any()).attribute("class", Pattern::any());
    let tbody = Element::new("tbody").attribute("style", Pattern::any()).attribute("class", Pattern::any());

    // Images & links
    let a = Element::new("a").attribute("style", Pattern::any()).attribute("class", Pattern::any())
        .attribute("href", Pattern::regex(Regex::new("^[^j\\s].+?$").unwrap()))
        .attribute("rel", Pattern::any()).attribute("target", Pattern::any());
    let img = Element::new("img").attribute("style", Pattern::any()).attribute("class", Pattern::any())
        .attribute("src", Pattern::any()).attribute("width", Pattern::any()).attribute("height", Pattern::any());

    let rules = Rules::new()
        .allow_comments(false)
        .element(p)
        .element(ul).element(ol).element(li)
        .element(b).element(strong).element(i).element(em).element(mark).element(small)
        .element(del).element(ins).element(sub).element(sup).element(br)
        .element(h1).element(h2).element(h3).element(h4).element(h5)
        .element(span).element(div).element(code).element(pre)
        .element(a).element(img)
        .element(table).element(tr).element(td).element(thead).element(tbody)
        .delete("style").delete("script").delete("object").delete("head").delete("link").delete("body")
        .delete("iframe").delete("applet").delete("comment").delete("embed").delete("listing").delete("meta")
        .delete("noscript").delete("plaintext").delete("xmp");
    return rules;
}

pub fn clean_html(input: &String, rules: &Rules) -> String {
    let sanitized = sanitize_str(rules, input).unwrap();
    return sanitized;
}
