-- Turns draft.md into a paper: the "Abstract" section becomes the abstract,
-- the byline under the title becomes the author, and the draft note is dropped.
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
    elseif i <= 3 and b.t == "Para" and pandoc.utils.stringify(b):match("junovhs@") then
      doc.meta.author = pandoc.MetaInlines(b.content)
    elseif b.t == "Para" and #b.content == 1 and b.content[1].t == "Emph"
        and pandoc.utils.stringify(b):match("^Draft") then
      -- the draft note
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
