all:
	convert -alpha off -background white -density 100 delta_id.pdf -strip -resize 800x1000 -extent 800x1000 PNG8:delta_id-%02d.png
	convert -layers OptimizePlus -delay 200 delta_id*.png -loop 0 delta_id.gif
	rm *.png
