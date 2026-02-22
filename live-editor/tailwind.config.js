/** @type {import('tailwindcss').Config} */
module.exports = {
  content: [
    "./src/**/*.{js,ts,html}",
    "./index.html",
  ],
  theme: {
    extend: {
      fontFamily: {
        hack: ['"Hack"', 'monospace'],
      },
      colors: {
        editor: {
          bg: '#1e1e1e',
          fg: '#d4d4d4',
          sidebar: '#252526',
          border: '#3e3e42',
        },
      },
    },
  },
  plugins: [],
}
