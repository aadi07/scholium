from fpdf import FPDF
pdf = FPDF()
pdf.add_page()
pdf.set_font("Arial", size=15)
pdf.cell(200, 10, txt="Hello World!", ln=1, align='C')
pdf.output("dummy.pdf")
