const dataDisplayDiv = document.getElementById('dataDisplayDiv');
const tableBody = document.getElementById('dataTableBody');
const dataTitleDisplay = document.getElementById('dataTitleDisplay');
const waitDiv = document.getElementById('waitDiv');
const backLinkDisplay = document.getElementById('backLinkDisplay');
const searchDisplayDiv = document.getElementById('searchDisplayDiv');
const searchBox = document.getElementById('searchBox');
const statDisplay = document.getElementById('statDisplay');
const statDisplayInner = document.getElementById('statDisplayInner');

let index = null;

async function fetch_index() {
    if (index != null) {
        return;
    }
    try {
        let response = await fetch('http://127.0.0.1:8080/api/index/');
        index = await response.json();
    } catch (e) {
        console.log(e);
        alert("Error fetching index");
    }
}

let expectedWindowHash = "";
window.onpopstate = async () => {
    const target = window.location.hash.substring(1);
    if (target === expectedWindowHash) {
        return;
    }
    if (target === "") {
        expectedWindowHash = target;
        await perform_search("");
        return;
    }
    await navigate_to_window_hash(target);
};

async function navigate_to_window_hash(target) {
    if (target.includes("/")) {
        statDisplay.classList.add("noDisplay");
        last_search_displayed = target;
        const [a, b] = target.split("/");
        await display_object(a, b);
    } else {
        await perform_search("");
        searchBox.value = "";
    }
}


let last_search_displayed = "";

async function perform_search(query) {
    waitDiv.classList.remove("noDisplay");
    dataDisplayDiv.classList.add("noDisplay");
    searchDisplayDiv.innerHTML = "";

    await fetch_index();

    if (query.length < 2) {
        window.location.hash = "";
        waitDiv.classList.add("noDisplay");
        statDisplay.classList.remove("noDisplay");
        return;
    }
    searchDisplayDiv.classList.remove("noDisplay");
    statDisplay.classList.add("noDisplay");

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
            display_search_result(result_category, result);
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

    waitDiv.classList.add("noDisplay");
    if (result_count === 1) {
        searchDisplayDiv.classList.add("noDisplay");
        if ((last_category + "/" + last_result) === last_search_displayed) {
            dataDisplayDiv.classList.remove("noDisplay");
            return;
        }
        last_search_displayed = last_category + "/" + last_result;
        await display_object(last_category, last_result, true);
    } else if (result_count === 0) {
        searchDisplayDiv.innerText = "No results";
    }
}

function display_search_result(object_type, object_name) {
    const div = document.createElement("div");
    const elem = document.createElement("a");
    elem.href = "#";
    elem.innerText = object_name;
    elem.onclick = async (ev) => {
        ev.preventDefault();
        searchDisplayDiv.classList.add("noDisplay");
        last_search_displayed = object_type + "/" + object_name;
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


async function display_object(object_type, object_name, no_set_search) {
    waitDiv.classList.remove("noDisplay");
    dataDisplayDiv.classList.add("noDisplay");
    tableBody.innerHTML = "";
    backLinkDisplay.innerHTML = "";
    dataTitleDisplay.innerText = "";
    const params = new URLSearchParams();
    params.set("name", object_name);
    params.set("type", object_type);
    let response = null;
    try {
        response = await (await fetch('http://127.0.0.1:8080/api/object/?' + params.toString())).json();
    } catch (e) {
        console.log(e);
        waitDiv.classList.add("noDisplay");
        alert("Error fetching object");
        return;
    }
    const name = response["category"] + "/" + response["object"]["filename"];

    expectedWindowHash = name;
    window.location.hash = name;

    if (no_set_search !== true) {
        searchBox.value = name;
    }
    dataTitleDisplay.innerText = name;
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
        console.log(line_no, entry);
        for (const link of forward_links) {
            const [link_line_no, link_target] = link;
            if (line_no !== link_line_no) {
                continue;
            }
            found_link = true;
            let [a, b] = link_target.split("/");
            const link_elem = document.createElement("a");
            link_elem.href = "#";
            if (a === object_type && b === object_name) {
                // link to self
                link_elem.style.color = "grey";
                link_elem.onclick = (ev) => {
                    ev.preventDefault();
                };
            } else {
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
            link_elem.href = "#";
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
    waitDiv.classList.add("noDisplay");
    dataDisplayDiv.classList.remove("noDisplay");
}

function* getBackLinks(back_links) {
    for (const entry of back_links) {
        yield entry.split("/");
    }
}

(async function main() {
    await get_stats();
    await navigate_to_window_hash(window.location.hash.substring(1));
}());

async function get_stats() {
    await fetch_index();
    for (const category of Object.entries(index)) {
        const elem = document.createElement("div");
        const href = document.createElement("a");
        href.href = "#";
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
    statDisplay.classList.remove("noDisplay");
    waitDiv.classList.add("noDisplay");
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
