/** @type {import('tailwindcss').Config} */
module.exports = {
  content: [
    "./src/**/*.{rs,html,js,jsx,ts,tsx}",
    "./dist/**/*.html",
  ],
  theme: {
    extend: {
      fontFamily: {
          hack: ['hack'],
      },
    },
  },
  plugins: [
      require('@tailwindcss/forms'),
  ],
}

