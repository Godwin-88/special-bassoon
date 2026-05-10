import React from 'react'

interface Props {
  content: string
  className?: string
}

export default function MarkdownRenderer({ content, className = '' }: Props) {
  const lines = content.split('\n')
  const elements: React.ReactNode[] = []
  let i = 0

  while (i < lines.length) {
    const line = lines[i]

    // Blank line
    if (line.trim() === '') {
      i++
      continue
    }

    // H1
    if (line.startsWith('# ')) {
      elements.push(<h1 key={i} className="text-base font-bold mt-3 mb-1">{parseInline(line.slice(2))}</h1>)
      i++
      continue
    }

    // H2
    if (line.startsWith('## ')) {
      elements.push(<h2 key={i} className="text-sm font-bold mt-3 mb-1">{parseInline(line.slice(3))}</h2>)
      i++
      continue
    }

    // H3
    if (line.startsWith('### ')) {
      elements.push(<h3 key={i} className="text-sm font-semibold mt-2 mb-0.5">{parseInline(line.slice(4))}</h3>)
      i++
      continue
    }

    // Numbered list: collect run
    if (/^\d+\.\s/.test(line)) {
      const items: React.ReactNode[] = []
      while (i < lines.length && /^\d+\.\s/.test(lines[i])) {
        const text = lines[i].replace(/^\d+\.\s/, '')
        items.push(<li key={i} className="ml-4 list-decimal">{parseInline(text)}</li>)
        i++
      }
      elements.push(<ol key={`ol-${i}`} className="space-y-0.5 my-1">{items}</ol>)
      continue
    }

    // Unordered list: collect run
    if (/^[-*]\s/.test(line)) {
      const items: React.ReactNode[] = []
      while (i < lines.length && /^[-*]\s/.test(lines[i])) {
        const text = lines[i].replace(/^[-*]\s/, '')
        items.push(<li key={i} className="ml-4 list-disc">{parseInline(text)}</li>)
        i++
      }
      elements.push(<ul key={`ul-${i}`} className="space-y-0.5 my-1">{items}</ul>)
      continue
    }

    // Horizontal rule
    if (/^---+$/.test(line.trim())) {
      elements.push(<hr key={i} className="my-2 border-gray-200 dark:border-gray-700" />)
      i++
      continue
    }

    // Paragraph
    elements.push(<p key={i} className="leading-relaxed">{parseInline(line)}</p>)
    i++
  }

  return <div className={`space-y-1 text-sm ${className}`}>{elements}</div>
}

// Parse inline markdown: **bold**, *italic*, `code`, and plain text
function parseInline(text: string): React.ReactNode[] {
  const parts: React.ReactNode[] = []
  // Pattern: **bold**, *italic*, `code`
  const re = /(\*\*(.+?)\*\*|\*(.+?)\*|`(.+?)`)/g
  let last = 0
  let match: RegExpExecArray | null

  while ((match = re.exec(text)) !== null) {
    if (match.index > last) {
      parts.push(text.slice(last, match.index))
    }
    if (match[0].startsWith('**')) {
      parts.push(<strong key={match.index} className="font-semibold">{match[2]}</strong>)
    } else if (match[0].startsWith('*')) {
      parts.push(<em key={match.index}>{match[3]}</em>)
    } else {
      parts.push(
        <code key={match.index} className="rounded bg-gray-100 dark:bg-gray-700 px-1 font-mono text-xs">
          {match[4]}
        </code>
      )
    }
    last = match.index + match[0].length
  }

  if (last < text.length) {
    parts.push(text.slice(last))
  }

  return parts.length > 0 ? parts : [text]
}
