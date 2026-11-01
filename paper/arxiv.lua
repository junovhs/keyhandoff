-- Turns paper.md into a paper: the "Abstract" section becomes the abstract,
-- and the byline under the title becomes the author.
function Pandoc(doc)
  local blocks, abstract, in_abstract = {}, {}, false
  for i, b in ipairs(doc.blocks) do
    if b.t == "Header" and b.level == 1 and pandoc.utils.stringify(b) == "Abstract" then
      in_abstract = true
    elseif b.t == "Header" and b.level <= 1 then
      in_abstract = false
      table.insert(blocks, b)
    elseif in_abstract then
      table.insert(abstract, b)
    elseif i <= 3 and b.t == "Para" and pandoc.utils.stringify(b):match("@") then
      -- "Name · email · link": the name on the first line, the rest under it.
      local name, rest, i = {}, {}, 1
      while i <= #b.content and not (b.content[i].t == "Str" and b.content[i].text == "·") do
        table.insert(name, b.content[i]); i = i + 1
      end
      for j = i + 2, #b.content do table.insert(rest, b.content[j]) end
      while #name > 0 and name[#name].t == "Space" do table.remove(name) end
      doc.meta["author-meta"] = pandoc.utils.stringify(name)
      if #rest > 0 then
        table.insert(name, pandoc.LineBreak())
        table.insert(name, pandoc.Span(rest, {class = "small"}))
      end
      doc.meta.author = pandoc.MetaInlines(name)
    else
      table.insert(blocks, b)
    end
  end
  doc.meta.abstract = pandoc.MetaBlocks(abstract)
  doc.blocks = blocks
  return doc
end

-- Section numbers are written in the headings; LaTeX shouldn't add its own.
function Header(h)
  h.classes:insert("unnumbered")
  return h
end
