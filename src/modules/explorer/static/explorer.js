const dataDisplayDiv = document.getElementById('dataDisplayDiv');
const tableBody = document.getElementById('dataTableBody');
const dataTitleDisplay = document.getElementById('dataTitleDisplay');
const waitDiv = document.getElementById('waitDiv');
const backLinkDisplay = document.getElementById('backLinkDisplay');
const searchDisplayDiv = document.getElementById('searchDisplayDiv');
const searchBox = document.getElementById('searchBox');
const statDisplayDiv = document.getElementById('statDisplayDiv');
const statDisplayInner = document.getElementById('statDisplayInner');
const errorDisplayDiv = document.getElementById('errorDisplayDiv');
const moreInfoLink = document.getElementById("moreInfoLink");
const infoDialog = document.getElementById("infoDialog");

let index = null;
let info = null;

async function fetch_index() {
    if (index != null) {
        return;
    }
    try {
        let response = await fetch('api/index/');
        const json = await response.json();
        info = json["i"];
        index = json["d"];
    } catch (e) {
        console.log(e);
        errorDisplayDiv.innerText = "Error fetching index";
        set_page_state("error");
    }
}

async function perform_search(query) {
    if (query.length < 2) {
        expectedWindowHash = "";
        window.location.hash = "";
        set_page_state("main");
        return;
    }

    set_page_state("wait");
    await fetch_index();

    searchDisplayDiv.innerHTML = "";
    set_page_state("search");
    const orig_query = query;

    let search_category = "";
    if (query.includes("/")) {
        const [a, b] = query.split("/");
        search_category = a;
        query = b;
    }
    query = query.toUpperCase();

    let result_count = 0;
    let target_result_count = 100;
    let last_result = null;
    let last_category = null;
    const gen = filter_results(search_category, query);

    function next_target_results() {
        while (target_result_count === -1 || result_count < target_result_count) {
            const {value, done} = gen.next();
            if (done) {
                break;
            }
            const [result_category, result] = value;
            result_count++;
            last_category = result_category;
            last_result = result;
            append_search_result(result_category, result);
        }
        if (result_count === target_result_count) {
            // Hit result limit
            const btnDiv = document.createElement("div");
            btnDiv.classList.add("showMoreBtnDiv");
            const moreBtn = document.createElement("button");
            moreBtn.innerText = "Show more..";
            moreBtn.onclick = () => {
                target_result_count += 100;
                moreBtn.disabled = true;
                btnDiv.parentNode.removeChild(btnDiv);
                next_target_results();
            };
            btnDiv.appendChild(moreBtn);
            const allBtn = document.createElement("button");
            allBtn.innerText = "Show all..";
            allBtn.onclick = () => {
                target_result_count = -1;
                allBtn.disabled = true;
                btnDiv.parentNode.removeChild(btnDiv);
                next_target_results();
            };
            btnDiv.appendChild(allBtn);
            searchDisplayDiv.appendChild(btnDiv);
        }
    }

    next_target_results();

    if (result_count === 1) {
        await display_object(last_category, last_result, true);
        return;
    } else if (result_count === 0) {
        searchDisplayDiv.innerText = "No results";
    }
    expectedWindowHash = "?" + orig_query;
    window.location.hash = "?" + orig_query;
}

function append_search_result(object_type, object_name) {
    const div = document.createElement("div");
    const elem = document.createElement("a");
    elem.href = `#/${get_object_path(object_type, object_name)}`;
    elem.innerText = object_name;
    elem.onclick = async (ev) => {
        ev.preventDefault();
        await display_object(object_type, object_name);
    };
    div.appendChild(elem);

    const badge = document.createElement("span");
    badge.classList.add("badge");
    badge.innerText = object_type;
    div.appendChild(badge);
    searchDisplayDiv.appendChild(div);
}

function* filter_results(search_category, query) {
    for (const category of Object.entries(index)) {
        if (search_category !== "" && category[0] !== search_category) {
            continue;
        }
        const filtered = category[1].filter((d) => {
            return d.toUpperCase().includes(query);
        });
        for (const result of filtered) {
            yield [category[0], result];
        }
    }
}

let last_displayed_object = "";

async function display_object(object_type, object_name, no_set_search) {
    const provided_obj_path = get_object_path(object_type, object_name);
    expectedWindowHash = "/" + provided_obj_path;
    if (window.location.hash !== ("#/" + provided_obj_path)) {
        window.location.hash = "/" + provided_obj_path;
    }
    if (last_displayed_object === provided_obj_path) {
        set_page_state("object");
        return;
    }
    set_page_state("wait");
    tableBody.innerHTML = "";
    backLinkDisplay.innerHTML = "";
    dataTitleDisplay.innerText = "";
    const params = new URLSearchParams();
    params.set("name", object_name);
    params.set("type", object_type);
    let response = null;
    try {
        response = await (await fetch('api/object/?' + params.toString())).json();
    } catch (e) {
        console.log(e);
        set_page_state("error");
        errorDisplayDiv.innerText = "Error fetching object";
        return;
    }
    last_displayed_object = provided_obj_path;

    const name = get_object_path(response["category"], response["object"]["filename"]);

    if (no_set_search !== true) {
        searchBox.value = name;
    }
    dataTitleDisplay.innerText = name;
    dataTitleDisplay.href = `#/${name}`
    dataTitleDisplay.onclick = (ev) => {
        ev.preventDefault();
        searchBox.value = name;
    };
    const key_value = response["object"]["key_value"];
    const forward_links = response["forward_links"];
    const back_links = response["back_links"];
    const combined = Object.entries(key_value).flatMap(([key, entries]) =>
        entries.map(entry => ({type: key, value: entry}))
    );
    combined.sort((a, b) => a.value[0] - b.value[0]);
    for (const entry of combined) {
        const {type: key, value: [_, value]} = entry;
        const trElem = document.createElement('tr');
        const tdElemKey = document.createElement('td');
        const tdElemValue = document.createElement('td');
        tdElemKey.innerText = key;
        let found_link = false;
        const line_no = entry.value[0];
        for (const link of forward_links) {
            const [link_line_no, link_target] = link;
            if (line_no !== link_line_no) {
                continue;
            }
            found_link = true;
            let [a, b] = link_target.split("/");
            const link_elem = document.createElement("a");
            if (a === object_type && b === object_name) {
                // link to self
                link_elem.href = `#/${get_object_path(a, b)}`;
                link_elem.style.color = "grey";
                link_elem.onclick = (ev) => {
                    ev.preventDefault();
                };
            } else {
                link_elem.href = `#/${get_object_path(a, b)}`;
                link_elem.onclick = (ev) => {
                    ev.preventDefault();
                    display_object(a, b);
                };
            }
            link_elem.innerText = value;
            tdElemValue.appendChild(link_elem);
            const badge = document.createElement("span");
            badge.classList.add("badge");
            badge.innerText = a;
            tdElemValue.appendChild(badge);
            break;
        }
        if (!found_link) {
            tdElemValue.innerText = value;
        }
        trElem.appendChild(tdElemKey);
        trElem.appendChild(tdElemValue);
        tableBody.appendChild(trElem);
    }

    const gen = getBackLinks(back_links);
    let back_link_count = 0;
    let target_back_link_count = 100;

    function next_target_back_links() {
        while (target_back_link_count === -1 || back_link_count < target_back_link_count) {
            const {value, done} = gen.next();
            if (done) {
                break;
            }
            back_link_count++;
            const [back_link_category, back_link_name] = value;
            const tr = document.createElement("tr");
            const td1 = document.createElement("td");
            const link_elem = document.createElement("a");
            link_elem.href = `#/${get_object_path(back_link_category, back_link_name)}`;
            link_elem.onclick = (ev) => {
                ev.preventDefault();
                display_object(back_link_category, back_link_name);
            };
            link_elem.innerText = back_link_name;
            td1.appendChild(link_elem);
            const badge = document.createElement("span");
            badge.classList.add("badge");
            badge.innerText = back_link_category;
            td1.appendChild(badge);
            tr.appendChild(td1);
            backLinkDisplay.appendChild(tr);
        }
        if (back_link_count === target_back_link_count) {
            // Hit result limit
            const btnDiv = document.createElement("div");
            btnDiv.classList.add("showMoreBtnDiv");
            const moreBtn = document.createElement("button");
            moreBtn.innerText = "Show more..";
            moreBtn.onclick = () => {
                moreBtn.disabled = true;
                target_back_link_count += 100;
                btnDiv.parentNode.removeChild(btnDiv);
                next_target_back_links();
            };
            btnDiv.appendChild(moreBtn);
            const allBtn = document.createElement("button");
            allBtn.innerText = "Show all..";
            allBtn.onclick = () => {
                allBtn.disabled = true;
                target_back_link_count = -1;
                btnDiv.parentNode.removeChild(btnDiv);
                next_target_back_links();
            };
            btnDiv.appendChild(allBtn);
            backLinkDisplay.appendChild(btnDiv);
        }
    }

    next_target_back_links();
    if (back_link_count === 0) {
        backLinkDisplay.innerText = "No references found";
    }
    set_page_state("object");
}

function* getBackLinks(back_links) {
    for (const entry of back_links) {
        yield entry.split("/");
    }
}

async function get_stats() {
    await fetch_index();
    for (const category of Object.entries(index)) {
        const elem = document.createElement("div");
        const href = document.createElement("a");
        href.href = `#?${category[0]}/`;
        href.innerText = category[0];
        href.onclick = (ev) => {
            ev.preventDefault();
            perform_search(category[0] + "/");
            searchBox.value = category[0] + "/";
        };
        const span = document.createElement("span");
        span.innerText = " - " + category[1].length;
        elem.appendChild(href);
        elem.appendChild(span);
        statDisplayInner.appendChild(elem);
    }
    set_page_state("main");
}

function set_page_state(state) {
    switch (state) {
        case "main":
            waitDiv.classList.add("noDisplay");
            dataDisplayDiv.classList.add("noDisplay");
            searchDisplayDiv.classList.add("noDisplay");
            statDisplayDiv.classList.remove("noDisplay");
            errorDisplayDiv.classList.add("noDisplay");
            break;
        case "search":
            waitDiv.classList.add("noDisplay");
            dataDisplayDiv.classList.add("noDisplay");
            searchDisplayDiv.classList.remove("noDisplay");
            statDisplayDiv.classList.add("noDisplay");
            errorDisplayDiv.classList.add("noDisplay");
            break;
        case "object":
            waitDiv.classList.add("noDisplay");
            dataDisplayDiv.classList.remove("noDisplay");
            searchDisplayDiv.classList.add("noDisplay");
            statDisplayDiv.classList.add("noDisplay");
            errorDisplayDiv.classList.add("noDisplay");
            break;
        case "wait":
            waitDiv.classList.remove("noDisplay");
            dataDisplayDiv.classList.add("noDisplay");
            searchDisplayDiv.classList.add("noDisplay");
            statDisplayDiv.classList.add("noDisplay");
            errorDisplayDiv.classList.add("noDisplay");
            break;
        case "error":
            waitDiv.classList.add("noDisplay");
            dataDisplayDiv.classList.add("noDisplay");
            searchDisplayDiv.classList.add("noDisplay");
            statDisplayDiv.classList.add("noDisplay");
            errorDisplayDiv.classList.remove("noDisplay");
            break;
    }
}

function get_object_path(object_type, object_name) {
    return `${object_type}/${object_name}`;
}


window.onpopstate = async () => {
    await handle_window_hash();
};

let expectedWindowHash = "";

async function handle_window_hash() {
    let target = window.location.hash.substring(1);
    target = decodeURI(target);
    if (target === expectedWindowHash) {
        return;
    }
    if (target.startsWith("?")) {
        target = target.substring(1);
        searchBox.value = target;
        await perform_search(target);
    } else if (target.startsWith("/")) {
        target = target.substring(1);
        if (target === "") {
            searchBox.value = "";
            set_page_state("main");
            return;
        }
        await navigate_to_window_hash(target);
    } else {
        searchBox.value = "";
        expectedWindowHash = "";
        set_page_state("main");
    }
}

async function navigate_to_window_hash(target) {
    set_page_state("wait");
    await fetch_index();
    let [a, b] = target.split("/");
    let found = false;
    for (const category of Object.entries(index)) {
        if (category[0] === a) {
            for (const item of category[1]) {
                if (item === b) {
                    found = true;
                    break;
                }
            }
            break;
        }
    }

    if (!found) {
        errorDisplayDiv.innerText = "Object linked to was not found";
        set_page_state("error");
        return;
    }
    await display_object(a, b);
}

let searchTimeout = null;
searchBox.oninput = () => {
    if (searchTimeout != null) {
        clearTimeout(searchTimeout);
    }
    searchTimeout = setTimeout(() => {
        perform_search(searchBox.value).then();
    }, 150);
};

moreInfoLink.onclick = async () => {
    const inner = document.getElementById("infoDialogInner");
    const inner_additional = document.getElementById("infoDialogAdditional");
    const close = document.getElementById("infoDialogCloseButton");
    inner.innerText = "Please wait...";
    infoDialog.showModal();
    close.onclick = () => {
        infoDialog.close();
    }
    await fetch_index();
    const with_roa = info["roa"];
    inner.innerText = `Registry git commit hash: ${info["commit"]}\nGeneration time: ${info["time"]}\nROA data generation enabled: ${with_roa}`;
    if (!with_roa) {
        inner_additional.classList.add("noDisplay");
    } else {
        inner_additional.classList.remove("noDisplay");
    }
}

(async function main() {
    await get_stats();
    await handle_window_hash();
}());